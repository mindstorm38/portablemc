from portablemc.standard import Context, make_standard_installer
from portablemc.task import Task, Watcher

from pathlib import Path


def test_install(tmp_path: Path):

    ctx = Context(tmp_path)

    installer = make_standard_installer(ctx, "1.19.3")
    installer.add_watcher(LogWatcher())
    
    print("installing...")
    installer.install()


class LogWatcher(Watcher):

    def on_begin(self, task: Task) -> None:
        print(f"begin: {type(task).__name__}", flush=True)
    
    def on_end(self, task: Task) -> None:
        print(f"end: {type(task).__name__}", flush=True)
    
    def on_event(self, name: str, **data) -> None:
        # print(f"event: {name}", data)
        pass
    
    def on_error(self, error: Exception) -> None:
        print(f"error: {error}", flush=True)


if __name__ == "__main__":
    test_install(Path(r"C:\Users\Theo\AppData\Roaming\.minecraft_test"))
