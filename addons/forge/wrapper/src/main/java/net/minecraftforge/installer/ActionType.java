package net.minecraftforge.installer;

import com.google.common.base.Predicate;

import java.io.File;

public interface ActionType {
	boolean run(File target, Predicate<String> optionals);
	boolean isPathValid(File paramFile);
	String getFileError(File paramFile);
	String getSuccessMessage();
	String getSponsorMessage();
}
