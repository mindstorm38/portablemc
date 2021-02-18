package portablemc.scripting;

import java.util.ArrayList;
import java.util.HashMap;

public class Main {
	
	private static final ArrayList<Object> objects = new ArrayList<>();
	private static final HashMap<Object, Integer> objectsIndices = new HashMap<>();
	
	public static void main(String[] args) {
	
	}
	
	private static Class<?> getClassUid(String fullName) {
		return classes.computeIfAbsent(fullName, name -> {
			try {
				return Class.forName(fullName);
			} catch (ClassNotFoundException e) {
				return null;
			}
		});
	}
	
	private static Object callClassStatic(String name, Object[] parameters) {
	
	}
	
	private static Object getClassStatic(String name) {
	
	}

}
