package argo.jdom;

import java.util.List;

final class JsonConstants extends JsonNode {
	
    static final JsonConstants NULL = new JsonConstants(JsonNodeType.NULL);
    static final JsonConstants TRUE = new JsonConstants(JsonNodeType.TRUE);
    static final JsonConstants FALSE = new JsonConstants(JsonNodeType.FALSE);

	private final JsonNodeType jsonNodeType;

    private JsonConstants(JsonNodeType jsonNodeType) {
        this.jsonNodeType = jsonNodeType;
    }

    public String getText() {
        return null;
    }

    @Override
    public List<JsonField> getFieldList() {
        return null;
    }
	
}
