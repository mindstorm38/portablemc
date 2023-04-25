package argo.jdom;

import java.util.List;

public abstract class JsonNode {
	
	public abstract String getText();
	
	public abstract List<JsonField> getFieldList();

	public JsonNode getNode(Object... pathElements) {
		return null;
	}

	public final List<JsonNode> getArrayNode(Object... pathElements) {
		return null;
	}
	
}
