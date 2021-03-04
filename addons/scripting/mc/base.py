from ..reflect import Runtime, Object, Wrapper
from .entity import LocalPlayer
from .gui import Gui

from typing import Optional


class Minecraft(Wrapper):

    type_name = "djz"

    def __init__(self, raw: Object):
        super().__init__(raw)
        rt = raw.get_runtime()
        class_minecraft = rt.types[Minecraft]
        self._field_player = class_minecraft.get_field("s", rt.types[LocalPlayer]) # player
        self._field_level = class_minecraft.get_field("r", rt.types[ClientLevel]) # level
        self._gui: Optional[Gui] = None

    @classmethod
    def get_instance(cls, rt: 'Runtime') -> 'Minecraft':
        class_minecraft = rt.types[Minecraft]
        field_instance = class_minecraft.get_field("F", class_minecraft)  # instance
        return Minecraft(field_instance.get_static())

    @property
    def player(self) -> 'Optional[LocalPlayer]':
        raw = self._field_player.get(self._raw)
        return None if raw is None else LocalPlayer(raw)

    @property
    def level(self) -> 'Optional[ClientLevel]':
        raw = self._field_level.get(self._raw)
        return None if raw is None else ClientLevel(raw)

    @property
    def gui(self) -> 'Gui':
        if self._gui is None:
            # This field is final in Minecraft's code, so we can cache it.
            class_minecraft = self.runtime.types[Minecraft]
            field_gui = class_minecraft.get_field("j", self.runtime.types[Gui]) # gui
            self._gui = Gui(field_gui.get(self._raw))
        return self._gui

    def __str__(self) -> str:
        return "<Minecraft>"