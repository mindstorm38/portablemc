from setuptools import setup

with open("README.md", "r", encoding="utf-8") as fp:
    long_description = fp.read()

setup(
    name="portablemc",
    version="1.2.0",
    description="PortableMC is a module that provides both an API for development of your custom launcher and an "
                "executable script to run PortableMC CLI.",
    author="Théo Rozier",
    author_email="contact@theorozier.fr",
    # py_modules=["portablemc"],
    packages=["portablemc"],
    url="https://github.com/mindstorm38/portablemc",
    long_description=long_description,
    long_description_content_type="text/markdown",
    license="GPL-3.0",
    egg_base="./build/lib"
)
