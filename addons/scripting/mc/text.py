from ..reflect import Runtime, Wrapper, MethodCache, ConstructorCache
from ..std import String
from typing import Any


__all__ = ["Component", "TextComponent"]


class Component(Wrapper):

    class_name = "nr"
    method_get_string = MethodCache(lambda: (Component, "getString"))

    @classmethod
    def ensure_component(cls, rt: 'Runtime', comp: Any) -> 'Component':
        if isinstance(comp, Component):
            return comp
        else:
            return TextComponent.new(rt, str(comp))

    def get_string(self) -> str:
        return self.method_get_string.invoke(self._raw)

    def __str__(self):
        return "<{} '{}'>".format(self.__class__.__name__, self.get_string())


class TextComponent(Component):

    class_name = "oe"
    constructor = ConstructorCache(lambda: (TextComponent, String))

    @classmethod
    def new(cls, rt: 'Runtime', text: str) -> 'TextComponent':
        return TextComponent(cls.constructor.construct(rt, text))
