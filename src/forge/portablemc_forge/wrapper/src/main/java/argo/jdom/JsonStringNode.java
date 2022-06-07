package argo.jdom;

import java.util.List;

public final class JsonStringNode extends JsonNode {
	
	private final String value;
	
	JsonStringNode(final String value) {
		this.value = value;
	}
	
	@Override
	public String getText() {
		return this.value;
	}
	
	@Override
	public List<JsonField> getFieldList() {
		return null;
	}
	
}
