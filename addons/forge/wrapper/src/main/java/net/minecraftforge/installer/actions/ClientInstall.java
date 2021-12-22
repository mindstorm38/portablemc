package net.minecraftforge.installer.actions;

import net.minecraftforge.installer.json.InstallV1;

import java.io.File;
import java.util.function.Predicate;

public class ClientInstall extends Action {

	public ClientInstall(InstallV1 profile, ProgressCallback monitor) { }
	
	@Override
	public boolean run(File target, Predicate<String> optionals, File installer) throws ActionCanceledException {
		return false;
	}
	
}
