package net.minecraftforge.installer;

import com.google.common.base.Predicate;

import java.io.File;

public class ClientInstall implements ActionType {
	
	@Override
	public boolean run(File target, Predicate<String> optionals) {
		return false;
	}
	
	@Override
	public boolean isPathValid(File paramFile) {
		return false;
	}
	
	@Override
	public String getFileError(File paramFile) {
		return null;
	}
	
	@Override
	public String getSuccessMessage() {
		return null;
	}
	
	@Override
	public String getSponsorMessage() {
		return null;
	}
	
}
