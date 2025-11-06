

from setuptools import setup, find_packages
from setuptools.command.install import install
from pathlib import Path
import os
import sys

with open('README.md') as readme_file:
    readme = readme_file.read()

with open('HISTORY.rst') as history_file:
    history = history_file.read()

with open('requirements.txt',encoding='utf-8') as requirements_file:
    all_pkgs = requirements_file.readlines()

requirements = [pkg.replace('\n', '') for pkg in all_pkgs if "#" not in pkg]
test_requirements = []

# Collect all files from agents and flows directories
def get_data_files(directory):
    """Recursively collect all files from a directory"""
    data_files = []
    for root, dirs, files in os.walk(directory):
        if files:
            rel_dir = os.path.relpath(root, '.')
            file_paths = [os.path.join(root, f) for f in files]
            data_files.append((rel_dir, file_paths))
    return data_files

data_files = []
if os.path.exists('agents'):
    data_files.extend(get_data_files('agents'))
if os.path.exists('flows'):
    data_files.extend(get_data_files('flows'))


class PostInstallCommand(install):
    """Post-installation for installation mode."""
    def run(self):
        install.run(self)

        # Check if ~/.local/bin is in PATH
        local_bin = os.path.expanduser('~/.local/bin')
        path_env = os.environ.get('PATH', '')

        if local_bin not in path_env and os.path.exists(local_bin):
            print("\n" + "="*70)
            print("IMPORTANT: MoFA CLI Setup Required")
            print("="*70)
            print(f"\nThe 'mofa' command was installed to: {local_bin}")
            print("This directory is NOT in your PATH.\n")
            print("To use 'mofa' command, please run:\n")

            shell = os.environ.get('SHELL', '')
            if 'bash' in shell:
                print(f"    echo 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.bashrc")
                print(f"    source ~/.bashrc\n")
            elif 'zsh' in shell:
                print(f"    echo 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.zshrc")
                print(f"    source ~/.zshrc\n")
            else:
                print(f"    export PATH=\"$HOME/.local/bin:$PATH\"\n")

            print("Alternative solutions:")
            print(f"  1. Use full path: {local_bin}/mofa --help")
            print(f"  2. Install in virtual environment (recommended):")
            print(f"     python3 -m venv .mofa && source .mofa/bin/activate && pip install mofa-core")
            print(f"  3. Use uv: uv venv .mofa && source .mofa/bin/activate && uv pip install mofa-core\n")
            print("="*70 + "\n")

setup(
    name='mofa-core',
    author='Cheng Chen, ZongHuan Wu',
    author_email='chenzi00103@gmail.com, zonghuan.wu@gmail.com',
    description='MoFA is a software framework for building AI agents through a composition-based approach.',
    python_requires='>=3.10',
    classifiers=[
        'Development Status :: 4 - Beta',
        'Intended Audience :: Developers',
        'Natural Language :: English',
        'Programming Language :: Python :: 3.10',
        'Programming Language :: Python :: 3.11',
        'Programming Language :: Python :: 3.12',
    ],
    entry_points={
        'console_scripts': [
            'mofa=mofa.cli:mofa_cli_group',
        ],
    },
    install_requires=requirements,
    long_description=readme + '\n\n' + history,
    include_package_data=True,
    keywords='mofa ai agents dataflow composition',
    packages=find_packages(include=['mofa', 'mofa.*']),
    long_description_content_type="text/markdown",
    package_dir={'mofa': 'mofa'},
    data_files=data_files,
    test_suite='tests',
    tests_require=test_requirements,
    version='0.1.32',
    zip_safe=False,
    dependency_links=[],
    cmdclass={
        'install': PostInstallCommand,
    }
)







