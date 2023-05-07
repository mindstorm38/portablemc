package portablemc.wrapper;

import argo.jdom.JsonNode;
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
		String version = VersionInfo.getMinecraftVersion();
		File versionDir = new File(versionRootDir, version);
		File versionJarFile = new File(versionDir, version + ".jar");
		return versionJarFile != null && versionJarFile.isFile() ? null : "no version jar file";
	}
	
	@Override
	public boolean install(File mainDir, String versionId) throws Exception {
		
		Path launcherProfile = WrapperUtil.ensureLauncherProfile(mainDir);
		
		ServerInstall.headless = true;  // Used by the buildMonitor method to return a wrapper around System.out
		
		// Changing version id is quite complicated in this version because we need to change the internal
		// JSON data through argo JDOM library which is an immutable one.
		JsonRootNode installProfile = VersionInfo.INSTANCE.versionData;

		Field stringNodeValueField = JsonStringNode.class.getDeclaredField("value");
		stringNodeValueField.setAccessible(true);
		Field fieldValueField = JsonField.class.getDeclaredField("value");
		fieldValueField.setAccessible(true);
		Field constFalseField = Class.forName("argo.jdom.JsonConstants").getDeclaredField("FALSE");
		constFalseField.setAccessible(true);
		JsonNode constFalse = (JsonNode) constFalseField.get(null);

		JsonStringNode targetNode = (JsonStringNode) installProfile.getNode("install", "target");
		String oldVersionId = targetNode.getText();
		stringNodeValueField.set(targetNode, versionId);

		// Here we disable downloads of the libraries, these will be downloaded by the launcher.
		for (JsonNode libNode : installProfile.getArrayNode("versionInfo", "libraries")) {
			JsonRootNode libObjNode = (JsonRootNode) libNode;
			for (JsonField libField : libObjNode.getFieldList()) {
				String fieldName = libField.getName().getText();
				if ("serverreq".equals(fieldName) || "clientreq".equals(fieldName)) {
					// Disable requirement.
					fieldValueField.set(libField, constFalse);
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
