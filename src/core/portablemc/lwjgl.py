"""Module providing a task for altering the LWJGL version of a launcher Minecraft version.
"""

from .task import Task, State, Watcher, Sequence
from .vanilla import FullMetadata, MetadataTask


class LwjglVersion:
    """This state specifies, if present, the LWJGL version to replace the current 
    full metadata with. This is usually used for supporting exotic systems like Arm,
    because the default metadata doesn't support such systems by default.
    """
    __slots__ = "version",
    def __init__(self, version: str) -> None:
        self.version = version


class LwjglVersionTask(Task):
    """Fix the full merged metadata by modifying the LWJGL versions used.

    This task alter the full metadata by replacing current LWJGL libraries by the one
    specified by the `LwjglVersion` state. The currently supported LWJGL versions are:
    3.2.3, 3.3.0, 3.3.1.

    *The current version of this task alter the metadata, but in the future this may be
    changed to be more efficient by directly modifying the resolved libraries state.*

    :in FullMetadata: Full metadata, altered by this task.
    :in LwjglVersion: Option, if present this task will trigger.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        lwjgl_version = state.get(LwjglVersion)
        if lwjgl_version is None:
            return
        
        lwjgl_version = lwjgl_version.version
        if lwjgl_version not in ("3.2.3", "3.3.0", "3.3.1"):
            raise ValueError(f"unsupported lwjgl fix version: {lwjgl_version}")

        metadata = state[FullMetadata].data

        lwjgl_libs = [
            "lwjgl",
            "lwjgl-jemalloc",
            "lwjgl-openal",
            "lwjgl-opengl",
            "lwjgl-glfw",
            "lwjgl-stb",
            "lwjgl-tinyfd",
        ]

        lwjgl_natives = {
            "windows": ["natives-windows", "natives-windows-x86"],
            "linux": ["natives-linux", "natives-linux-arm64", "natives-linux-arm32"],
            "osx": ["natives-macos"]
        }

        if lwjgl_version in ("3.3.0", "3.3.1"):
            lwjgl_natives["windows"].append("natives-windows-arm64")
            lwjgl_natives["osx"].append("natives-macos-arm64")
        
        metadata_libs = metadata["libraries"]

        libraries_to_remove = []
        for idx, lib_obj in enumerate(metadata_libs):
            if "name" in lib_obj and lib_obj["name"].startswith("org.lwjgl:"):
                libraries_to_remove.append(idx)

        for idx_to_remove in reversed(libraries_to_remove):
            metadata_libs.pop(idx_to_remove)

        maven_repo_url = "https://repo1.maven.org/maven2"

        for lwjgl_lib in lwjgl_libs:

            lib_path = f"org/lwjgl/{lwjgl_lib}/{lwjgl_version}/{lwjgl_lib}-{lwjgl_version}.jar"
            lib_url = f"{maven_repo_url}/{lib_path}"
            lib_name = f"org.lwjgl:{lwjgl_lib}:{lwjgl_version}"

            metadata_libs.append({
                "downloads": {
                    "artifact": {
                        "path": lib_path,
                        "url": lib_url
                    }
                },
                "name": lib_name
            })

            for lwjgl_os, lwjgl_classifiers in lwjgl_natives.items():
                for lwjgl_classifier in lwjgl_classifiers:
                    classifier_path = f"org/lwjgl/{lwjgl_lib}/{lwjgl_version}/{lwjgl_lib}-{lwjgl_version}-{lwjgl_classifier}.jar"
                    classifier_url = f"{maven_repo_url}/{classifier_path}"
                    metadata_libs.append({
                        "downloads": {
                            "artifact": {
                                "path": classifier_path,
                                "url": classifier_url
                            }
                        },
                        "name": f"{lib_name}:{lwjgl_classifier}",
                        "rules": [{"action": "allow", "os": {"name": lwjgl_os}}]
                    })
        
        watcher.handle(LwjglVersionEvent(lwjgl_version))


class LwjglVersionEvent:
    __slots__ = "version",
    def __init__(self, version: str) -> None:
        self.version = version


def add_lwjgl_tasks(seq: Sequence) -> None:
    """Add the tasks required by modifying the given sequence. This is used with the
    `LwjglVersion` state, if present the LWJGL version will be adjusted.
    """
    seq.append_task(LwjglVersionTask(), after=MetadataTask)
