package portablemc.wrapper;

import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.List;

public class InstallRunnerType {
	
	public final String runnerClassName;
	public final String[] requiredClassNames;
	
	public InstallRunnerType(String runnerClassName, String[] requiredClassNames) {
		this.runnerClassName = runnerClassName;
		this.requiredClassNames = requiredClassNames;
	}
	
	//
	
	private static final List<InstallRunnerType> TYPES = new ArrayList<>();
	
	static {
		
		// The order of insertion is really important here, because some installer have both V1 and V2
		// procedures, but only the V1 actually works in those installers.
		
		TYPES.add(new InstallRunnerType("portablemc.wrapper.V1InstallRunner", new String[]{
				"net.minecraftforge.installer.ClientInstall",
				"net.minecraftforge.installer.ServerInstall",
				"argo.jdom.JsonNode",
				"javax.swing.JOptionPane"
		}));
		
		TYPES.add(new InstallRunnerType("portablemc.wrapper.V2InstallRunner", new String[]{
				"net.minecraftforge.installer.actions.ClientInstall"
		}));
		
	}
	
	@SuppressWarnings("unchecked")
	public static InstallRunner findAndBuildRunner() {
		for (InstallRunnerType type : TYPES) {
			try {
				for (String requiredClassName : type.requiredClassNames) {
					Class.forName(requiredClassName);
				}
				Class<? extends InstallRunner> runnerClass = (Class<? extends InstallRunner>) Class.forName(type.runnerClassName);
				Constructor<? extends InstallRunner> runnerConstructor = runnerClass.getDeclaredConstructor();
				return runnerConstructor.newInstance();
			} catch (ReflectiveOperationException ignored) { }
		}
		return null;
	}
	
}
