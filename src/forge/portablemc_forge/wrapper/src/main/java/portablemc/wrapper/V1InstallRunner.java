package portablemc.wrapper;

import argo.jdom.JsonField;
import argo.jdom.JsonRootNode;
import argo.jdom.JsonStringNode;
import net.minecraftforge.installer.ClientInstall;
import net.minecraftforge.installer.ServerInstall;
import net.minecraftforge.installer.VersionInfo;

import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * This installation procedure can call JOptionPane.showMessageDialog, we check some
 * parameter to avoid these calls but some calls cannot be avoided. One solution might
 * be to internally redefine this class inside the final JAR.
 */
public class V1InstallRunner extends InstallRunner {
	
	@Override
	public String validate(File mainDir) {
		if (!mainDir.isDirectory()) return "no main dir";
		File versionRootDir = new File(mainDir, "versions");
		File minecraftJarFile = VersionInfo.getMinecraftFile(versionRootDir);
		return minecraftJarFile != null && minecraftJarFile.isFile() ? null : "no version jar file";
	}
	
	@Override
	public boolean install(File mainDir, String versionId) throws Exception {
		
		Path launcherProfile = WrapperUtil.ensureLauncherProfile(mainDir);
		
		ServerInstall.headless = true;  // Used by the buildMonitor method to return a wrapper around System.out
		
		String oldVersionId = versionId;
		
		// Changing version id is quite complicated in this version because we need to change the internal
		// JSON data through argo JDOM library which is an immutable one.
		JsonRootNode installProfile = VersionInfo.INSTANCE.versionData;
		outer: for (JsonField field0 : installProfile.getFieldList()) {
			if ("install".equals(field0.getName().getText())) {
				for (JsonField field1 : field0.getValue().getFieldList()) {
					if ("target".equals(field1.getName().getText())) {
						JsonStringNode targetNode = (JsonStringNode) field1.getValue();
						oldVersionId = targetNode.getText();
						Field field = JsonStringNode.class.getDeclaredField("value");
						field.setAccessible(true);
						field.set(targetNode, versionId);
						break outer;
					}
				}
			}
		}
		
		ClientInstall install = new ClientInstall();
		
		boolean success = install.run(mainDir, a -> true);
		
		WrapperUtil.fixVersionMetaId(mainDir, oldVersionId, versionId);
		
		if (launcherProfile != null) {
			// We force run the garbage collector to automatically close the reader that
			// is not closed by some versions of the installer. Which prevents deletion.
			// However, I can't know if this will work every time.
			System.gc();
			Files.delete(launcherProfile);
		}
		
		return success;
		
	}
	
}
