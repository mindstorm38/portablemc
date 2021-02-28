package portablemc.scripting;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.Inet4Address;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;

public class ScriptingClient implements Runnable {
	
	public static void main(String[] args) {
	
		String scriptingMain = System.getProperty("portablemc.scripting.main");
		String rawPort = System.getProperty("portablemc.scripting.port");
		int port = 0;
		
		if (scriptingMain == null) {
			System.err.println("No scripting main class to call, please register property 'portablemc.scripting.main'.");
			System.exit(1);
		} else if (rawPort == null) {
			System.err.println("No scripting port, please specify the server port using 'portablemc.scripting.port'.");
			System.exit(1);
		} else {
			try {
				port = Integer.parseInt(rawPort);
			} catch (NumberFormatException e) {
				System.err.println("Invalid scripting server port '" + rawPort + "'.");
				System.exit(1);
			}
		}
		
		ScriptingClient client = new ScriptingClient(port);
		Thread thread = new Thread(client, "PortableMC Scripting Client Thread");
		thread.setDaemon(true);
		thread.start();
		
		try {
			Class<?> clazz = Class.forName(scriptingMain);
			Method method = clazz.getMethod("main", String[].class);
			method.invoke(clazz, new Object[]{args});
		} catch (ReflectiveOperationException e) {
			System.err.println("Main class not found or invalid as entry point.");
			e.printStackTrace();
			System.exit(1);
		}
		
		try {
			client.stop();
			thread.join(1000);
		} catch (InterruptedException e) {
			e.printStackTrace();
		}
		
	}
	
	private static final byte PACKET_GET_CLASS = 1;
	private static final byte PACKET_GET_FIELD = 2;
	private static final byte PACKET_GET_METHOD = 3;
	private static final byte PACKET_FIELD_GET = 10;
	private static final byte PACKET_FIELD_SET = 11;
	private static final byte PACKET_METHOD_INVOKE = 20;
	private static final byte PACKET_RESULT = 30;
	
	private static final HashMap<String, Class<?>> PRIMITIVE_TYPES = new HashMap<>();
	
	static {
		PRIMITIVE_TYPES.put("byte", byte.class);
		PRIMITIVE_TYPES.put("short", short.class);
		PRIMITIVE_TYPES.put("int", int.class);
		PRIMITIVE_TYPES.put("long", long.class);
		PRIMITIVE_TYPES.put("float", float.class);
		PRIMITIVE_TYPES.put("double", double.class);
		PRIMITIVE_TYPES.put("boolean", boolean.class);
		PRIMITIVE_TYPES.put("char", char.class);
	}
	
	private final int port;
	private final Socket socket;
	private final ArrayList<Object> objects = new ArrayList<>();
	private final HashMap<Object, Integer> objectsIndices = new HashMap<>();
	
	private final ByteBuffer txBuf = ByteBuffer.allocate(4096);
	private final ByteBuffer rxBuf = ByteBuffer.allocate(4096);
	private OutputStream txStream;
	
	public ScriptingClient(int port) {
		this.port = port;
		this.socket = new Socket();
	}
	
	public void stop() {
		try {
			this.socket.close();
		} catch (IOException e) {
			e.printStackTrace();
		}
	}
	
	@Override
	public void run() {
		
		try {
			
			print("Connecting to server at 127.0.0.1:" + this.port + "...");
			this.socket.connect(new InetSocketAddress(Inet4Address.getByName("127.0.0.1"), this.port));
			print("Connected!");
			
			InputStream rxStream = this.socket.getInputStream();
			this.txStream = this.socket.getOutputStream();
			
			ByteBuffer rxBuf = this.rxBuf;
			rxBuf.clear();
			
			//byte[] receivedBuffer = new byte[4096];
			//int receivedLength = 0;
			int readLength, rxPos;
			
			//this.rxBuf = ByteBuffer.wrap(receivedBuffer);
			
			//int nextPacketType = 0;
			int nextPacketLength = 0;
			
			while (!this.socket.isClosed()) {
				
				/*while ((readLength = rxStream.read(receivedBuffer, receivedLength, receivedBuffer.length - receivedLength)) > 0) {
					receivedLength += readLength;
				}*/
				
				
				/*rxPos = rxBuf.position();
				while ((readLength = rxStream.read(rxBuf.array(), rxPos, rxBuf.remaining())) > 0) {
					rxPos += readLength;
					rxBuf.position(rxPos);
					print("new buffer content: " + Arrays.toString(Arrays.copyOf(rxBuf.array(), rxPos)));
				}*/
				
				rxPos = rxBuf.position();
				
				if (nextPacketLength == 0 && rxPos >= 3) {
					//nextPacketType = this.rxBuf.get(0);
					nextPacketLength = Short.toUnsignedInt(rxBuf.getShort(1)) + 3; // +3 for the header
					print("next packet length: " + nextPacketLength);
				}
				
				if (nextPacketLength != 0 && nextPacketLength >= rxPos) {
					
					//rxBuf.clear();
					rxBuf.limit(nextPacketLength);
					rxBuf.position(3);
					
					byte packetType = rxBuf.get(0);
					
					print("decoding packet: " + packetType);
					
					try {
						this.decodePacket(packetType);
					} catch (IOException e) {
						print("Failed to decode packet " + packetType);
						e.printStackTrace();
					}
					
					byte[] rxData = rxBuf.array();
					System.arraycopy(rxData, nextPacketLength, rxData, 0, rxData.length - nextPacketLength);
					
					//receivedLength -= nextPacketLength;
					rxBuf.clear();
					rxBuf.position(rxPos - nextPacketLength);
					//nextPacketType = 0;
					nextPacketLength = 0;
					
				} else {
					
					print("polling...");
					if ((readLength = rxStream.read(rxBuf.array(), rxPos, rxBuf.remaining())) > 0) {
						rxBuf.position(rxPos + readLength);
					}
					
				}
				
			}
			
			print("Scripting client stopped!");
			
		} catch (IOException e) {
			e.printStackTrace();
		}
		
	}
	
	// Packet encoding-decoding //
	
	private void preparePacket() {
		this.txBuf.clear();
		this.txBuf.position(this.txBuf.position() + 3); // Reserved for packet length
	}
	
	private void sendPacket(byte packetType) throws IOException {
		int len = this.txBuf.position();
		this.txBuf.put(0, packetType);
		this.txBuf.putShort(1, (short) (len - 3));
		this.txStream.write(this.txBuf.array(), 0, len);
	}
	
	private void decodePacket(int packetType) throws IOException {
		
		ByteBuffer rxBuf = this.rxBuf;
		this.preparePacket();
		
		if (packetType == PACKET_GET_CLASS) {
			this.putIndex(this.ensureCachedClass(getString(rxBuf)));
		} else if (packetType == PACKET_GET_FIELD) {
			
			int classIdx = rxBuf.getInt();
			String fieldName = getString(rxBuf);
			int typeClassIdx = rxBuf.getInt();
			Class<?> typeClass = this.getCachedObjectChecked(typeClassIdx, Class.class);
			
			if (typeClass == null) {
				this.putNull();
			} else {
				this.putIndex(this.ensureCachedField(classIdx, fieldName, typeClass));
			}
			
		} else if (packetType == PACKET_GET_METHOD) {
			
			int classIdx = rxBuf.getInt();
			String methodName = getString(rxBuf);
			int paramsCount = Byte.toUnsignedInt(rxBuf.get());
			Class<?>[] parameterTypes = new Class[paramsCount];
			
			for (int i = 0; i < paramsCount; ++i) {
				Class<?> clazz = this.getCachedObjectChecked(rxBuf.getInt(), Class.class);
				if (clazz == null) {
					classIdx = -1;
					break;
				}
				parameterTypes[i] = clazz;
			}
			
			if (classIdx == -1) {
				this.putNull();
			} else {
				this.putIndex(this.ensureCachedMethod(classIdx, methodName, parameterTypes));
			}
			
		} else if (packetType == PACKET_FIELD_GET) {
			
			int fieldIdx = rxBuf.getInt();
			Object ownerObj = this.getValue();
			this.putValue(this.getCachedFieldValue(fieldIdx, ownerObj));
			
		} else if (packetType == PACKET_FIELD_SET) {
			
			int fieldIdx = rxBuf.getInt();
			Object ownerObj = this.getValue();
			Object valueObj = this.getValue();
			this.setCachedFieldValue(fieldIdx, ownerObj, valueObj);
			this.putNull();
		
		} else if (packetType == PACKET_METHOD_INVOKE) {
			
			int methodIdx = rxBuf.getInt();
			Object ownerObj = this.getValue();
			int paramsCount = Byte.toUnsignedInt(rxBuf.get());
			Object[] parameterValues = new Object[paramsCount];
			
			for (int i = 0; i < paramsCount; ++i) {
				parameterValues[i] = this.getValue();
			}
			
			this.putValue(this.invokeCachedMethod(methodIdx, ownerObj, parameterValues));
			
		} else {
			print("Illegal packet type: " + packetType);
			return;
		}
		
		this.sendPacket(PACKET_RESULT);
		
	}
	
	private Object getValue() {
		ByteBuffer rxBuf = this.rxBuf;
		int objIndex = rxBuf.getInt();
		if (objIndex < 0) {
			switch (objIndex) {
				case -2:
					return rxBuf.get();
				case -3:
					return rxBuf.getShort();
				case -4:
					return rxBuf.getInt();
				case -5:
					return rxBuf.getLong();
				case -6:
					return rxBuf.getFloat();
				case -7:
					return rxBuf.getDouble();
				case -8:
					return rxBuf.getChar();
				case -9:
					return getString(rxBuf);
				case -10:
					return Boolean.FALSE;
				case -11:
					return Boolean.TRUE;
				default:
					return null;
			}
		} else {
			return this.getCachedObjectChecked(objIndex, Object.class);
		}
	}
	
	private void putValue(Object value) {
		ByteBuffer txBuf = this.txBuf;
		if (value == null) {
			txBuf.putInt(-1);
		} else {
			Class<?> clazz = value.getClass();
			if (clazz == Byte.class) {
				txBuf.putInt(-2);
				txBuf.put((Byte) value);
			} else if (clazz == Short.class) {
				txBuf.putInt(-3);
				txBuf.putShort((Short) value);
			} else if (clazz == Integer.class) {
				txBuf.putInt(-4);
				txBuf.putInt((Integer) value);
			} else if (clazz == Long.class) {
				txBuf.putInt(-5);
				txBuf.putLong((Long) value);
			} else if (clazz == Float.class) {
				txBuf.putInt(-6);
				txBuf.putFloat((Float) value);
			} else if (clazz == Double.class) {
				txBuf.putInt(-7);
				txBuf.putDouble((Double) value);
			} else if (clazz == Character.class) {
				txBuf.putInt(-8);
				txBuf.putDouble((Character) value);
			} else if (clazz == String.class) {
				txBuf.putInt(-9);
				putString(txBuf, (String) value);
			} else if (clazz == Boolean.class) {
				if ((boolean) value) {
					txBuf.putInt(-11);
				} else {
					txBuf.putInt(-10);
				}
			} else {
				txBuf.putInt(this.ensureCachedObject(value));
			}
		}
	}
	
	private void putIndex(int index) {
		this.txBuf.putInt(index);
	}
	
	private void putNull() {
		this.txBuf.putInt(-1);
	}
	
	// Cached objects //
	
	private int ensureCachedObject(Object object) {
		return this.objectsIndices.computeIfAbsent(object, obj -> {
			int idx = this.objects.size();
			this.objects.add(obj);
			return idx;
		});
	}
	
	private <T> T getCachedObjectChecked(int idx, Class<? extends T> clazz) {
		if (idx < 0 || idx >= this.objects.size()) {
			print("No " + clazz.getSimpleName() + " indexed at " + idx);
			return null;
		}
		try {
			return clazz.cast(this.objects.get(idx));
		} catch (ClassCastException e) {
			print("Object with id " + idx + " is not a " + clazz.getSimpleName());
			return null;
		}
	}
	
	private int ensureCachedClass(String className) {
		try {
			Class<?> clazz = PRIMITIVE_TYPES.get(className);
			return this.ensureCachedObject(clazz == null ? Class.forName(className) : clazz);
		} catch (ClassNotFoundException e) {
			print("Class not found: " + className);
			return -1;
		}
	}
	
	private int ensureCachedMethod(int classIdx, String methodName, Class<?>[] parameterTypes) {
		Class<?> clazz = this.getCachedObjectChecked(classIdx, Class.class);
		if (clazz == null) {
			return -1;
		}
		try {
			Method method = clazz.getDeclaredMethod(methodName, parameterTypes);
			method.setAccessible(true);
			return this.ensureCachedObject(method);
		} catch (NoSuchMethodException e) {
			print("Can't find method " + clazz.getSimpleName() + "." + methodName + Arrays.toString(parameterTypes));
			return -1;
		}
	}
	
	private int ensureCachedField(int classIdx, String fieldName, Class<?> typeClass) {
		Class<?> clazz = this.getCachedObjectChecked(classIdx, Class.class);
		if (clazz == null) {
			return -1;
		}
		try {
			Field field = clazz.getDeclaredField(fieldName);
			if (field.getType() == typeClass) {
				field.setAccessible(true);
				return this.ensureCachedObject(field);
			} else {
				print("Field " + field + " has not the expected type " + typeClass.getSimpleName() + ", got " + field.getType().getSimpleName());
				return -1;
			}
		} catch (NoSuchFieldException e) {
			print("Can't find field " + clazz.getSimpleName() + "." + fieldName);
			return -1;
		}
	}
	
	private Object getCachedFieldValue(int fieldIdx, Object ownerObj) {
		Field field = this.getCachedObjectChecked(fieldIdx, Field.class);
		if (field == null) {
			return null;
		}
		try {
			return field.get(ownerObj);
		} catch (IllegalAccessException e) {
			print("Can't get field value " + field);
			return null;
		}
	}
	
	private void setCachedFieldValue(int fieldIdx, Object ownerObj, Object value) {
		Field field = this.getCachedObjectChecked(fieldIdx, Field.class);
		if (field == null) {
			return;
		}
		try {
			field.set(ownerObj, value);
		} catch (IllegalAccessException e) {
			print("Can't get field value " + field);
		}
	}
	
	private Object invokeCachedMethod(int methodIdx, Object ownerObj, Object[] parameterValues) {
		Method method = this.getCachedObjectChecked(methodIdx, Method.class);
		if (method == null) {
			return null;
		}
		try {
			return method.invoke(ownerObj, parameterValues);
		} catch (IllegalAccessException | InvocationTargetException e) {
			print("Can't find method " + method);
			return null;
		}
	}
	
	// UTF encode utilities //
	
	private static void putString(ByteBuffer dst, String src) {
		byte[] bytes = src.getBytes(StandardCharsets.UTF_8);
		dst.putShort((short) bytes.length);
		dst.put(bytes);
	}
	
	private static String getString(ByteBuffer src) {
		int length = Short.toUnsignedInt(src.getShort());
		int pos = src.position();
		String res = new String(src.array(), pos, length, StandardCharsets.UTF_8);
		src.position(pos + length);
		return res;
	}
	
	// Print output //
	
	private static void print(String msg) {
		System.out.println("[SCRIPTING] " + msg);
	}
	
}
