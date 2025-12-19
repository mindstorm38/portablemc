use std::io::{self, Read, Write};
use std::fs::{self, File};

use portablemc::download::{self, Batch, Entry, EntryErrorKind};

use tempfile::TempDir;

use mockito::{Mock, Matcher, Server, ServerGuard};


struct TestBatch {
    inner: Batch,
    server: ServerGuard,
    dir: TempDir,
}

impl TestBatch {

    pub fn new() -> Self {
        Self {
            inner: Batch::new(),
            server: Server::new(),
            dir: tempfile::Builder::new()
                .prefix("")
                .suffix(".download")
                .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
                .unwrap(),
        }
    }

    pub fn push(&mut self, path: &str) -> (Mock, &mut Entry) {

        let mock = self.server.mock("GET", &*format!("/{path}"));
        let mut url = self.server.url();
        url.push('/');
        url.push_str(path);

        let file = self.dir.path().join(path);
        let entry = self.inner.push(url, file);

        (mock, entry)
        
    }

}


#[test]
fn all() {

    let mut batch = TestBatch::new();

    let entry = batch.push("success");
    entry.0
        .with_status(200)
        .with_body("Hello world!")
        .create();

    let entry = batch.push("error_reqwest_decode");
    entry.0
        .with_status(200)
        .with_chunked_body(|_| {
            Err(io::ErrorKind::TimedOut.into())
        })
        .create();

    let entry = batch.push("error_invalid_code");
    entry.0
        .with_status(400)
        .create();

    let entry = batch.push("error_invalid_size");
    entry.0
        .with_status(200)
        .with_body("Hello wo..")
        .create();
    entry.1
        .set_expected_size(Some(12));

    let entry = batch.push("error_invalid_sha1");
    entry.0
        .with_status(200)
        .with_body("Hello wo..")
        .create();
    entry.1
        .set_expected_sha1(Some(*b"\xd3\x48\x6a\xe9\x13\x6e\x78\x56\xbc\x42\x21\x23\x85\xea\x79\x70\x94\x47\x58\x02"));

    // The invalid size error should trigger first!
    let entry = batch.push("error_invalid_size_and_sha1");
    entry.0
        .with_status(200)
        .with_body("Hello wo..")
        .create();
    entry.1
        .set_expected_size(Some(12))
        .set_expected_sha1(Some(*b"\xd3\x48\x6a\xe9\x13\x6e\x78\x56\xbc\x42\x21\x23\x85\xea\x79\x70\x94\x47\x58\x02"));

    // 304 is invalid if cache is not enable or if the file is not yet cached!
    let entry = batch.push("error_not_modified");
    entry.0
        .with_status(304)
        .with_body("Hello world!")
        .create();

    // Test the keep open feature and that the file's cursor is properly placed.
    let entry = batch.push("success_with_file");
    entry.0
        .with_status(200)
        .with_body("Hello world!")
        .create();
    entry.1
        .set_keep_open();

    // We check that no cache is created if we don't return Etag, or Last-Modified 
    // headers.
    let entry = batch.push("success_not_cached");
    entry.0
        .with_status(200)
        .with_body("Hello world!")
        .create();
    entry.1
        .set_use_cache();
    
    let mut batch_result = batch.inner.download(()).unwrap();

    // Basic successful entry...
    let result = batch_result.entry(0).unwrap();
    assert!(result.file().is_file());
    assert_eq!(result.size(), 12);
    assert_eq!(result.sha1(), b"\xd3\x48\x6a\xe9\x13\x6e\x78\x56\xbc\x42\x21\x23\x85\xea\x79\x70\x94\x47\x58\x02");
    assert!(result.handle().is_none());

    // Checking errors...
    let result = batch_result.entry(1).unwrap_err();
    match result.kind() {
        EntryErrorKind::Internal(err) => {
            assert!(err.is::<reqwest::Error>());
        }
        e => panic!("{e:?}")
    }

    assert!(matches!(batch_result.entry(2).unwrap_err().kind(), EntryErrorKind::InvalidStatus(400)));
    assert!(matches!(batch_result.entry(3).unwrap_err().kind(), EntryErrorKind::InvalidSize));
    assert!(matches!(batch_result.entry(4).unwrap_err().kind(), EntryErrorKind::InvalidSha1));
    assert!(matches!(batch_result.entry(5).unwrap_err().kind(), EntryErrorKind::InvalidSize));
    assert!(matches!(batch_result.entry(6).unwrap_err().kind(), EntryErrorKind::InvalidStatus(304)));

    for i in 1..=6 {
        let result = batch_result.entry(i).unwrap_err();
        assert!(!result.file().exists(), "{} should not exist", result.file().display());
    }

    // Success with keep open...
    let result = batch_result.entry_mut(7).unwrap();
    let handle = result.handle_mut().unwrap();
    let mut result_from_file = String::new();
    handle.read_to_string(&mut result_from_file).unwrap();
    assert_eq!(result_from_file, "Hello world!");

    // Success with cache, but not cached because of missing headers...
    let result = batch_result.entry(8).unwrap();
    assert!(result.file().is_file());
    let mut path = result.file().to_path_buf();
    path.as_mut_os_string().push(".cache");
    assert!(!path.exists());

}

#[test]
fn cache() {

    let mut server = Server::new();
    
    // Choose a temporary file...
    let url = format!("{}/cached", server.url());
    let file = tempfile::Builder::new()
        .prefix("")
        .suffix(".download")
        .tempfile_in(env!("CARGO_TARGET_TMPDIR"))
        .unwrap()
        .into_temp_path();

    // ...and deduce its cache file.
    let cache_file = tempfile::TempPath::from_path({
        let mut buf = file.to_path_buf();
        buf.as_mut_os_string().push(".cache");
        buf
    });

    // Without prior caching, it should initialize the cache with the Etag
    {
        let mock = server.mock("GET", "/cached")
            .with_status(200)
            .with_header("Etag", "0123456789")
            .with_header("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_body("Hello world!")
            .create();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world!");
    }
    
    // With the cached Etag, we return 304
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", "0123456789")
            .with_status(304)
            .create();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world!");
    }
    
    // With the cached Last-Modified, we return 304
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-Modified-Since", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_status(304)
            .create();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world!");
    }
    
    // Now we do like the Etag has changed
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", "0123456789")
            .match_header("If-Modified-Since", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_status(200)
            .with_header("Etag", "0123456789v2")
            .with_header("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_body("Hello world! v2")
            .create();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();
        
        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world! v2");
    }
    
    // Now we do like the Last-Modified has changed
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", "0123456789v2")
            .match_header("If-Modified-Since", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_status(200)
            .with_header("Etag", "0123456789v2")
            .with_header("Last-Modified", "Sun, 06 Nov 1994 08:49:50 GMT")
            .with_body("Hello world! v3")
            .create();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world! v3");
    }
    
    // We check that if the file has been modified, it is checked against the cache 
    // metadata and so it is re-downloaded without giving the headers.
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", Matcher::Missing)
            .match_header("If-Modified-Since", Matcher::Missing)
            .with_status(200)
            .with_header("Etag", "0123456789")
            .with_header("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_body("Hello world!")
            .create();

        File::options()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(b"__unexpected__")
            .unwrap();

        download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Hello world!");
    }
    
    // Now we just want to check that keep open from a cached file is correct.
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", "0123456789")
            .match_header("If-Modified-Since", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_status(304)
            .create();

        let mut result = download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .set_keep_open()
            .download(())
            .unwrap();
        assert!(file.is_file());
        assert!(cache_file.is_file());
        mock.assert();

        let mut buf = String::new();
        result.take_handle()
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        assert_eq!(buf, "Hello world!");
    }
    
    // Now we just want to check that expected size is checked, if we returned unexpected
    // content then it should return an error and delete the file and its cache 
    // information.
    {
        let mock = server.mock("GET", "/cached")
            .match_header("If-None-Match", "0123456789")
            .match_header("If-Modified-Since", "Sun, 06 Nov 1994 08:49:37 GMT")
            .with_status(200)
            .with_body("Hello world!__unexpected__")
            .with_header("Etag", "0123456789")
            .with_header("Last-Modified", "Sun, 06 Nov 1994 08:49:37 GMT")
            .create();

        let _result = download::single(url.clone(), file.to_path_buf())
            .set_use_cache()
            .set_expected_size(Some(12))
            .download(())
            .unwrap_err();
        assert!(!file.exists());
        assert!(!cache_file.exists());
        mock.assert();
    }

}
