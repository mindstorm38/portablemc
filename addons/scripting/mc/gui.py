from ..reflect import Wrapper, MethodCache
from .text import Component

from typing import Any, Optional


__all__ = ["Gui", "Chat"]


class Gui(Wrapper):

    class_name = "dkv"
    method_set_overlay_message = MethodCache(lambda: (Gui, "a", Component, "boolean")) # setOverlayMessage
    method_set_titles = MethodCache(lambda: (Gui, "a", Component, Component, "int", "int", "int")) # setTitles
    method_get_chat = MethodCache(lambda: (Gui, "c"))

    def __init__(self, raw):
        super().__init__(raw)
        self._chat: 'Optional[Chat]' = None

    def set_overlay(self, comp: Any, animate_color: bool = False):
        comp = Component.ensure_component(self.runtime, comp)
        self.method_set_overlay_message.invoke(self._raw, comp.raw, animate_color)

    def set_title(self, *, title: Optional[Any] = None, subtitle: Optional[Any] = None, fade_in: int = -1, stay: int = -1, fade_out: int = -1):

        if fade_in != -1 or stay != -1 or fade_out != -1:
            self.method_set_titles.invoke(self._raw, None, None, fade_in, stay, fade_out)

        if title is not None:
            title = Component.ensure_component(self.runtime, title)
            self.method_set_titles.invoke(self._raw, title.raw, None, -1, -1, -1)

        if subtitle is not None:
            subtitle = Component.ensure_component(self.runtime, subtitle)
            self.method_set_titles.invoke(self._raw, None, subtitle.raw, -1, -1, -1)

    @property
    def chat(self) -> 'Chat':
        if self._chat is None:
            self._chat = Chat(self.method_get_chat.invoke(self._raw))
        return self._chat

    def __str__(self):
        return "<Gui>"


class Chat(Wrapper):

    class_name = "dlk" # ChatComponent
    method_clear_messages = MethodCache(lambda: (Chat, "a", "boolean"))
    method_add_message = MethodCache(lambda: (Chat, "a", Component, "int"))
    method_remove_message = MethodCache(lambda: (Chat, "b", "int"))

    def clear_messages(self, *, clear_history: bool = False):
        self.method_clear_messages.invoke(self._raw, clear_history)

    def add_message(self, comp: Any, *, msg_id: int = 0):
        comp = Component.ensure_component(self.runtime, comp)
        self.method_add_message.invoke(self._raw, comp.raw, msg_id)

    def remove_message(self, msg_id: int):
        self.method_remove_message.invoke(self._raw, msg_id)

    def __str__(self):
        return "<Chat>"
