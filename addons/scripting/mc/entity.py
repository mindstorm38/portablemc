from ..reflect import Wrapper, FieldCache, MethodCache
from ..std import Enum
from .text import Component
import enum


__all__ = ["EntityPose", "Entity", "LivingEntity", "Player", "AbstractClientPlayer", "LocalPlayer"]


class EntityPose(enum.Enum):

    STANDING = 0
    FALL_FLYING = 1
    SLEEPING = 2
    SWIMMING = 3
    SPIN_ATTACK = 4
    CROUCHING = 5
    DYING = 6


class Entity(Wrapper):

    type_name = "aqa" # Entity
    field_x = FieldCache(lambda: (Entity, "m", "double")) # xo
    field_y = FieldCache(lambda: (Entity, "n", "double")) # yo
    field_z = FieldCache(lambda: (Entity, "o", "double")) # zo
    method_get_pose = MethodCache(lambda: (Entity, "ae")) # getPose
    method_get_name = MethodCache(lambda: (Entity, "R")) # getName
    method_get_type_name = MethodCache(lambda: (Entity, "bJ")) # getTypeName

    @property
    def x(self) -> float:
        return self.field_x.get(self._raw)

    @property
    def y(self) -> float:
        return self.field_y.get(self._raw)

    @property
    def z(self) -> float:
        return self.field_z.get(self._raw)

    @property
    def pose(self) -> EntityPose:
        raw_enum = self.method_get_pose.invoke(self._raw)
        if raw_enum is None:
            return EntityPose.STANDING
        else:
            try:
                return EntityPose[Enum(raw_enum).name]
            except KeyError:
                return EntityPose.STANDING

    @property
    def name(self) -> Component:
        return Component(self.method_get_name.invoke(self._raw))

    @property
    def type_name(self) -> Component:
        return Component(self.method_get_type_name.invoke(self._raw))

    def __str__(self):
        return "<{}>".format(self.__class__.__name__)


class LivingEntity(Entity):
    type_name = "aqm"


class Player(LivingEntity):
    type_name = "bfw"


class AbstractClientPlayer(Player):
    type_name = "dzj"


class LocalPlayer(Player):
    type_name = "dzm"
