package net.minecraftforge.installer.actions;

import java.io.OutputStream;

public interface ProgressCallback {
	
	static ProgressCallback withOutputs(OutputStream...streams) {
		return null;
	}
	
}
