package portablemc.wrapper;

import java.io.BufferedOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import java.security.cert.X509Certificate;
import java.security.cert.CertificateException;
import java.security.NoSuchAlgorithmException;
import java.security.KeyManagementException;
import java.security.SecureRandom;

import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSession;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.HostnameVerifier;

public final class WrapperUtil {

	public static Path ensureLauncherProfile(File mainDir) throws IOException {
		Path launcherProfile = mainDir.toPath().resolve("launcher_profiles.json");
		if (!Files.exists(launcherProfile)) {
			BufferedOutputStream launcherProfileOut = new BufferedOutputStream(Files.newOutputStream(launcherProfile));
			launcherProfileOut.write("{\"profiles\":{}}".getBytes());
			launcherProfileOut.close();
			return launcherProfile;
		}
		return null;
	}
	
	public static void fixVersionMetaId(File mainDir, String oldVersionId, String newVersionId) throws IOException {
		Path versionMetaFile = mainDir.toPath().resolve("versions").resolve(newVersionId).resolve(newVersionId + ".json");
		String content = new String(Files.readAllBytes(versionMetaFile)).replaceFirst(oldVersionId, newVersionId);
		Files.write(versionMetaFile, content.getBytes());
	}
	
}
