package net.minecraftforge.installer.actions;

public class ActionCanceledException extends Exception {
	ActionCanceledException(Exception parent) {
		super(parent);
	}
}
