package portablemc.wrapper;

import java.io.BufferedOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

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
