from os import path


def main():

    print("Generating PortableMC Core script... ", end='')

    this_dir = path.dirname(__file__)
    pmc_file = path.join(this_dir, "portablemc.py")
    pmc_core_file = path.join(this_dir, "portablemc_core.py")

    with open(pmc_file, "rt") as src:
        with open(pmc_core_file, "wt") as dst:
            while True:
                line = src.readline()
                if not len(line) or line.startswith("if __name__ == '__main__':"):
                    break
                dst.write(line)

    print("Done.")

if __name__ == '__main__':
    main()
