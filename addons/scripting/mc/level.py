from ..reflect import Object, Wrapper, FieldCache, MethodCache
from .entity import AbstractClientPlayer
from ..std.util import BaseList


class LevelData(Wrapper):
    type_name = "cyd"


class WritableLevelData(LevelData):
    type_name = "cyo"


class Level(Wrapper):

    type_name = "brx"
    field_level_data = FieldCache(lambda: (Level, "u", WritableLevelData)) # fieldData
    method_get_game_time = MethodCache(lambda: (LevelData, "e")) # getGameTime()
    method_get_day_time = MethodCache(lambda: (LevelData, "f")) # getDayTime()
    method_is_raining = MethodCache(lambda: (LevelData, "k")) # isRaining()
    method_is_thundering = MethodCache(lambda: (LevelData, "i")) # isThundering()
    __slots__ = "_level_data"

    def __init__(self, raw: Object):
        super().__init__(raw)
        self._level_data = self.field_level_data.get(self.runtime).get(raw)

    @property
    def game_time(self) -> int:
        return self.method_get_game_time.get(self.runtime).invoke(self._level_data)

    @property
    def day_time(self) -> int:
        return self.method_get_day_time.get(self.runtime)(self._level_data)

    @property
    def is_raining(self) -> int:
        return self.method_is_raining.get(self.runtime)(self._level_data)

    @property
    def is_thundering(self) -> int:
        return self.method_is_thundering.get(self.runtime)(self._level_data)


class ClientLevel(Level):

    type_name = "dwt"
    method_get_players = MethodCache(lambda: (ClientLevel, "x")) # players()

    def get_players(self) -> 'BaseList':
        return BaseList(self.method_get_players.get(self.runtime).invoke(self._raw), AbstractClientPlayer)
