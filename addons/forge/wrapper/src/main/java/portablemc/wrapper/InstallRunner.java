package portablemc.wrapper;

import java.io.File;

public abstract class InstallRunner {
	public abstract String validate(File mainDir);
	public abstract boolean install(File mainDir, String versionId) throws Exception;
}
