from typing import Optional, Tuple, Union, Callable, Any


__all__ = [
    "ReflectError", "ClassNotFoundError", "FieldNotFoundError", "MethodNotFoundError",
    "Runtime", "AnyType",
    "Object",
    "Class", "ClassMember",
    "Field",
    "Executable", "Method", "Constructor",
    "FieldCache", "MethodCache", "ConstructorCache",
    "Wrapper"
]


class ReflectError(Exception): ...
class ClassNotFoundError(ReflectError): ...
class FieldNotFoundError(ReflectError): ...
class MethodNotFoundError(ReflectError): ...


class Runtime:

    """
    Base class for a reflection runtime.
    """

    def __init__(self):
        self._types = Types(self)

    def get_class_from_name(self, name: str) -> 'Class':
        raise NotImplementedError

    def get_class_from_object(self, obj: 'Object') -> 'Class':
        raise NotImplementedError

    def get_class_field_from_name(self, cls: 'Class', name: str, field_type: 'Class') -> 'Field':
        raise NotImplementedError

    def get_class_method_from_name(self, cls: 'Class', name: str, parameter_types: 'Tuple[Class, ...]') -> 'Method':
        raise NotImplementedError

    def get_class_constructor_from_name(self, cls: 'Class', parameter_types: 'Tuple[Class, ...]') -> 'Constructor':
        raise NotImplementedError

    def get_field_value(self, field: 'Field', owner: 'Optional[Object]') -> 'AnyType':
        raise NotImplementedError

    def set_field_value(self, field: 'Field', owner: 'Optional[Object]', value: 'AnyType'):
        raise NotImplementedError

    def invoke_method(self, method: 'Method', owner: 'Optional[Object]', parameters: 'Tuple[AnyType, ...]') -> 'AnyType':
        raise NotImplementedError

    def invoke_constructor(self, constructor: 'Constructor', parameters: 'Tuple[AnyType, ...]') -> 'Object':
        raise NotImplementedError

    @property
    def types(self) -> 'Types':
        return self._types


class Types:

    def __init__(self, rt: 'Runtime'):
        self._rt = rt

    def _get_class(self, item) -> 'Class':
        if hasattr(item, "type_name"):
            item = item.type_name
        return self._rt.get_class_from_name(str(item))

    def __getattr__(self, item) -> 'Class':
        return self._get_class(item)

    def __getitem__(self, item) -> 'Class':
        return self._get_class(item)

    def __str__(self):
        return "<Types>"


class Object:

    """
    An concrete binding to a runtime's object, this is different from object
    wrappers and this class and its subclasses are only used for reflection.
    """

    __slots__ = "_rt", "_ptr", "_cls"

    def __init__(self, rt: 'Runtime', ptr: int):
        self._rt = rt
        self._ptr = ptr
        self._cls: 'Optional[Class]' = None

    def get_runtime(self) -> 'Runtime':
        return self._rt

    def get_pointer(self) -> int:
        return self._ptr

    def get_class(self) -> 'Class':
        if self._cls is None:
            self._cls = self._rt.get_class_from_object(self)
        return self._cls

    def __str__(self):
        return "<Object @{:08X}>".format(self._ptr)


AnyType = Union[Object, int, float, bool, str, None]


class Class(Object):

    __slots__ = "_name",

    def __init__(self, rt: 'Runtime', ptr: int, name: str):
        super().__init__(rt, ptr)
        self._name = name

    def get_name(self) -> str:
        return self._name

    def get_field(self, name: str, field_type: 'Class') -> 'Field':
        return self._rt.get_class_field_from_name(self, name, field_type)

    def get_method(self, name: str, *parameter_types: 'Class') -> 'Method':
        return self._rt.get_class_method_from_name(self, name, parameter_types)

    def get_constructor(self, *parameter_types: 'Class') -> 'Constructor':
        return self._rt.get_class_constructor_from_name(self, parameter_types)

    def is_primitive(self) -> bool:
        return self._name in ("byte", "short", "int", "long", "float", "double", "boolean", "char")

    def __str__(self):
        return "<Class {}>".format(self._name)


class ClassMember(Object):

    __slots__ = "_owner", "_name"

    def __init__(self, rt: 'Runtime', ptr: int, owner: Class, name: str):
        super().__init__(rt, ptr)
        self._owner = owner
        self._name = name


class Field(ClassMember):

    __slots__ = "_type"

    def __init__(self, rt: 'Runtime', ptr: int, owner: Class, name: str, field_type: Class):
        super().__init__(rt, ptr, owner, name)
        self._type = field_type

    def get_type(self) -> 'Class':
        return self._type

    def get(self, owner: 'Optional[Object]') -> 'AnyType':
        return self._rt.get_field_value(self, owner)

    def set(self, owner: 'Optional[Object]', value: 'AnyType'):
        self._rt.set_field_value(self, owner, value)

    def get_static(self) -> 'AnyType':
        return self.get(None)

    def set_static(self, value: 'AnyType'):
        self.set(None, value)

    def __str__(self):
        return "<Field {} {}.{}>".format(self._type.get_name(), self._owner.get_name(), self._name)


class Executable(ClassMember):

    __slots__ = "_parameter_types"

    def __init__(self, rt: Runtime, ptr: int, owner: 'Class', name: str, parameter_types: 'Tuple[Class, ...]'):
        super().__init__(rt, ptr, owner, name)
        self._parameter_types = parameter_types

    def get_parameter_types(self) -> 'Tuple[Class, ...]':
        return self._parameter_types


class Method(Executable):

    __slots__ = ()

    def __init__(self, rt: Runtime, ptr: int, owner: 'Class', name: str, parameter_types: 'Tuple[Class, ...]'):
        super().__init__(rt, ptr, owner, name, parameter_types)

    def invoke(self, owner: 'Optional[Object]', *parameters: 'AnyType') -> 'AnyType':
        return self._rt.invoke_method(self, owner, parameters)

    def invoke_static(self, *parameters: 'AnyType') -> 'AnyType':
        return self.invoke(None, *parameters)

    def __str__(self):
        return "<Method {}.{}({})>".format(
            self._owner.get_name(),
            self._name,
            ", ".format(*(typ.get_name for typ in self._parameter_types))
        )


class Constructor(Executable):

    def __init__(self, rt: Runtime, ptr: int, owner: 'Class', parameter_types: 'Tuple[Class, ...]'):
        super().__init__(rt, ptr, owner, "<init>", parameter_types)

    def construct(self, *parameters: 'AnyType') -> 'Object':
        return self._rt.invoke_constructor(self, parameters)

    def __str__(self):
        return "<Method {}({})>".format(
            self._owner.get_name(),
            ", ".format(*(typ.get_name for typ in self._parameter_types))
        )


# Class wrapper and utilities for it

class Wrapper:

    """
    Common wrapper class used to interpret raw reflection objects.
    """

    type_name = "java.lang.Object"
    __slots__ = "_raw"

    def __init__(self, raw: 'Object'):
        if raw is None:
            raise ValueError("Can't wrap null object.")
        self._raw = raw

    @property
    def raw(self) -> 'Object':
        return self._raw

    @property
    def runtime(self) -> 'Runtime':
        return self._raw.get_runtime()

    def __str__(self):
        return f"<Wrapped {self.type_name}>"


class MemberCache:

    __slots__ = "_member", "_supplier"

    def __init__(self, supplier: 'Callable[[], tuple]'):
        self._supplier = supplier
        self._member: 'Optional[ClassMember]' = None

    def ensure(self, rt: 'Runtime') -> Any:
        if self._member is None or self._member.get_runtime() is not rt:
            self._member = self._provide(rt, *self._supplier())
        return self._member

    def _provide(self, rt: 'Runtime', *args) -> 'ClassMember':
        raise NotImplementedError


class FieldCache(MemberCache):

    def ensure(self, rt: 'Runtime') -> 'Field':
        return super().ensure(rt)

    def _provide(self, rt: 'Runtime', *args) -> 'Field':
        class_name, field_name, field_type_name = args
        cls = rt.types[class_name]
        return cls.get_field(field_name, rt.types[field_type_name])

    def get(self, owner: 'Object') -> 'AnyType':
        return self.ensure(owner.get_runtime()).get(owner)

    def set(self, owner: 'Object', value: 'AnyType'):
        self.ensure(owner.get_runtime()).set(owner, value)

    def get_static(self, rt: 'Runtime') -> 'AnyType':
        return self.ensure(rt).get_static()

    def set_static(self, rt: 'Runtime', value: 'AnyType'):
        self.ensure(rt).set_static(value)


class MethodCache(MemberCache):

    def ensure(self, rt: 'Runtime') -> 'Method':
        return super().ensure(rt)

    def _provide(self, rt: 'Runtime', *args) -> 'Method':
        class_name, method_name, *parameter_types = args
        cls = rt.types[class_name]
        return cls.get_method(method_name, *(rt.types[param_type] for param_type in parameter_types))

    def invoke(self, owner: 'Object', *parameters: 'AnyType') -> 'AnyType':
        return self.ensure(owner.get_runtime()).invoke(owner, *parameters)

    def invoke_static(self, rt: 'Runtime', *parameters: 'AnyType') -> 'AnyType':
        return self.ensure(rt).invoke_static(*parameters)


class ConstructorCache(MemberCache):

    def ensure(self, rt: 'Runtime') -> 'Constructor':
        return super().ensure(rt)

    def _provide(self, rt: 'Runtime', *args) -> 'Constructor':
        class_name, *parameter_types = args
        cls = rt.types[class_name]
        return cls.get_constructor(*(rt.types[param_type] for param_type in parameter_types))

    def construct(self, rt: 'Runtime', *parameters: 'AnyType') -> 'Object':
        return self.ensure(rt).construct(*parameters)
