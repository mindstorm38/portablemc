from ..reflect import Wrapper, MethodCache
from .text import Component

from typing import Any, Optional


__all__ = ["Gui"]


class Gui(Wrapper):

    type_name = "dkv"
    method_set_overlay_message = MethodCache(lambda: (Gui, "a", Component, "boolean")) # setOverlayMessage
    method_set_titles = MethodCache(lambda: (Gui, "a", Component, Component, "int", "int", "int")) # setTitles

    def set_overlay(self, comp: Any, animate_color: bool = False):
        comp = Component.ensure_component(self.runtime, comp)
        self.method_set_overlay_message.get(self.runtime)(self._raw, comp.raw, animate_color)

    def set_title(self, *, title: Optional[Any] = None, subtitle: Optional[Any] = None, fade_in: int = -1, stay: int = -1, fade_out: int = -1):

        if fade_in != -1 or stay != -1 or fade_out != -1:
            self.method_set_titles.get(self.runtime)(self._raw, None, None, fade_in, stay, fade_out)

        if title is not None:
            title = Component.ensure_component(self.runtime, title)
            self.method_set_titles.get(self.runtime)(self._raw, title.raw, None, -1, -1, -1)

        if subtitle is not None:
            subtitle = Component.ensure_component(self.runtime, subtitle)
            self.method_set_titles.get(self.runtime)(self._raw, None, subtitle.raw, -1, -1, -1)
