package net.minecraftforge.installer;

import argo.jdom.JsonRootNode;

import java.io.File;

public class VersionInfo {
	
	public static final VersionInfo INSTANCE = new VersionInfo();
	
	public final JsonRootNode versionData;
	
	public VersionInfo() {
		this.versionData = null;
	}
	
	public static File getMinecraftFile(File path) {
		return null;
	}
	
}
