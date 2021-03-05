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

    class_name = "aqa" # Entity
    field_x = FieldCache(lambda: (Entity, "m", "double")) # xo
    field_y = FieldCache(lambda: (Entity, "n", "double")) # yo
    field_z = FieldCache(lambda: (Entity, "o", "double")) # zo
    method_get_pose = MethodCache(lambda: (Entity, "ae")) # getPose
    method_get_name = MethodCache(lambda: (Entity, "R")) # getName
    method_get_type_name = MethodCache(lambda: (Entity, "bJ")) # getTypeName
    method_set_shared_flag = MethodCache(lambda: (Entity, "b", "int", "boolean")) # setSharedFlag
    method_is_glowing = MethodCache(lambda: (Entity, "bE")) # isGlowing
    method_set_glowing = MethodCache(lambda: (Entity, "i", "boolean")) # setGlowing

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
    def glowing(self) -> bool:
        return self.method_is_glowing.invoke(self._raw)

    @glowing.setter
    def glowing(self, glowing: bool):
        # self.method_set_glowing.invoke(self._raw, glowing)
        self.method_set_shared_flag.invoke(self._raw, 6, glowing)
        # This setter is not reliable for unknown reason.

    @property
    def name(self) -> Component:
        return Component(self.method_get_name.invoke(self._raw))

    @property
    def type_name(self) -> Component:
        return Component(self.method_get_type_name.invoke(self._raw))

    def __str__(self):
        return "<{}>".format(self.__class__.__name__)


class LivingEntity(Entity):
    class_name = "aqm"


class Player(LivingEntity):
    class_name = "bfw"


class AbstractClientPlayer(Player):
    class_name = "dzj"


class LocalPlayer(Player):
    class_name = "dzm"
