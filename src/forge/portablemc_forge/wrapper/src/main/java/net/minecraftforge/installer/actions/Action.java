package net.minecraftforge.installer.actions;

import java.io.File;
import java.util.function.Predicate;

public abstract class Action {
	
	public abstract boolean run(File target, Predicate<String> optionals, File installer) throws ActionCanceledException;
	
}
