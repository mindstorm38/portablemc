from ..reflect import Runtime, Object, Wrapper, FieldCache
from ..std import Queue, Runnable

from .entity import LocalPlayer
from .level import ClientLevel
from .gui import Gui

from typing import Optional


class Minecraft(Wrapper):

    class_name = "djz"
    field_player = FieldCache(lambda: (Minecraft, "s", LocalPlayer)) # player
    field_level = FieldCache(lambda: (Minecraft, "r", ClientLevel)) # level
    field_progress_tasks = FieldCache(lambda: (Minecraft, "aU", Queue)) # progressTasks

    def __init__(self, raw: Object):
        super().__init__(raw)
        self._gui: Optional[Gui] = None
        self._progress_tasks: Optional[Queue] = None

    @classmethod
    def get_instance(cls, rt: 'Runtime') -> 'Minecraft':
        class_minecraft = rt.types[Minecraft]
        field_instance = class_minecraft.get_field("F", class_minecraft)  # instance
        return Minecraft(field_instance.get_static())

    @property
    def player(self) -> 'Optional[LocalPlayer]':
        raw = self.field_player.get(self._raw)
        return None if raw is None else LocalPlayer(raw)

    @property
    def level(self) -> 'Optional[ClientLevel]':
        raw = self.field_level.get(self._raw)
        return None if raw is None else ClientLevel(raw)

    @property
    def gui(self) -> 'Gui':
        if self._gui is None:
            # This field is final in Minecraft's code, so we can cache it.
            class_minecraft = self.runtime.types[Minecraft]
            field_gui = class_minecraft.get_field("j", self.runtime.types[Gui]) # gui
            self._gui = Gui(field_gui.get(self._raw))
        return self._gui

    def _get_progress_tasks(self) -> Queue:
        # Progress tasks is a queue of Runnables, this queue can be used
        # to synchronize critical method calls. This queue is thread-safe.
        if self._progress_tasks is None:
            self._progress_tasks = Queue(self.field_progress_tasks.get(self._raw), Runnable)
        return self._progress_tasks

    

    def __str__(self) -> str:
        return "<Minecraft>"