package portablemc.wrapper;

import net.minecraftforge.installer.SimpleInstaller;
import net.minecraftforge.installer.actions.ClientInstall;
import net.minecraftforge.installer.actions.ProgressCallback;
import net.minecraftforge.installer.json.Install;
import net.minecraftforge.installer.json.InstallV1;
import net.minecraftforge.installer.json.Util;

import java.io.File;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;

public class V2InstallRunner extends InstallRunner {
	
	@Override
	public String validate(File mainDir) {
		return mainDir.isDirectory() ? null : "no main dir";
	}
	
	@Override
	public boolean install(File mainDir, String versionId) throws Exception {
		
		Path launcherProfile = WrapperUtil.ensureLauncherProfile(mainDir);
		
		InstallV1 profile = Util.loadInstallProfile();
		
		Field versionField = Install.class.getDeclaredField("version");
		versionField.setAccessible(true);
		String oldVersionId = (String) versionField.get(profile);
		versionField.set(profile, versionId);
		
		ProgressCallback monitor = ProgressCallback.withOutputs(System.out);
		
		SimpleInstaller.headless = true;
		
		File installer = new File(SimpleInstaller.class.getProtectionDomain().getCodeSource().getLocation().toURI());
		ClientInstall install = new ClientInstall(profile, monitor);
		
		boolean success = install.run(mainDir, a -> true, installer);
		
		// This file should exist after the installation.
		WrapperUtil.fixVersionMetaId(mainDir, oldVersionId, versionId);
		
		if (launcherProfile != null) {
			Files.delete(launcherProfile);
		}
		
		return success;
		
	}
	
}
