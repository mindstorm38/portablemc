package portablemc.wrapper;

import java.io.File;

public class Main {
	
	public static void main(String[] args) throws Exception {
		
		if (args.length != 2) {
			System.out.println("format: ... <main_dir> <version_id>");
			System.exit(1);
		}
		
		File mainDir = new File(args[0]);
		
		if (!mainDir.isDirectory()) {
			System.out.println("error: invalid main directory");
			System.exit(2);
		}
		
		InstallRunner runner = InstallRunnerType.findAndBuildRunner();
		
		if (runner == null) {
			System.out.println("error: cannot find an install runner");
			System.exit(3);
		} else {
			System.out.println("info: using install runner " + runner.getClass().getSimpleName());
		}
		
		String validation = runner.validate(mainDir);
		if (validation != null) {
			System.out.println("error: install runner not runnable: " + validation);
			System.exit(4);
		} else {
			System.out.println("info: install runner validated main directory");
		}
		
		if (!runner.install(mainDir, args[1])) {
			System.out.println("error: installation failed");
			System.exit(5);
		} else {
			System.out.println("info: installation successful");
		}
		
		System.exit(0);
	
	}

}
