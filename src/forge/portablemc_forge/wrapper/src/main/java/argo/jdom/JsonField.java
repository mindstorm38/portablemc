package argo.jdom;

public class JsonField {
	
	private final JsonStringNode name;
	private final JsonNode value;
	
	public JsonField(final JsonStringNode name, final JsonNode value) {
		this.name = name;
		this.value = value;
	}
	
	public JsonStringNode getName() {
		return this.name;
	}
	
	public JsonNode getValue() {
		return this.value;
	}
	
}
