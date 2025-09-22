

from setuptools import setup, find_packages

with open('README.rst') as readme_file:
    readme = readme_file.read()

with open('HISTORY.rst') as history_file:
    history = history_file.read()

with open('requirements.txt',encoding='utf-8') as requirements_file:
    all_pkgs = requirements_file.readlines()

requirements = [pkg.replace('\n', '') for pkg in all_pkgs if "#" not in pkg]
test_requirements = []

setup(
    name='mofa-ai',
    author='Cheng Chen, ZongHuan Wu',
    author_email='chenzi00103@gmail.com, zonghuan.wu@gmail.com',
    description='MoFA is a software framework for building AI agents through a composition-based approach.',
    python_requires='>=3.10',
    classifiers=[
        'Development Status :: 2 - Pre-Alpha',
        'Intended Audience :: Developers',
        'Natural Language :: English',
        'Programming Language :: Python :: 3.10',
    ],
    entry_points={
        'console_scripts': [
            'mofa=mofa.cli:mofa_cli_group',
        ],
    },
    install_requires=requirements,
    long_description=readme + '\n\n' + history,
    include_package_data=True,
    keywords='mofa',
    packages=find_packages(include=['mofa', 'mofa.*']),
    long_description_content_type="text/markdown",
    package_dir={'mofa': 'mofa'},
    test_suite='tests',
    tests_require=test_requirements,
    version='0.2.8',
    zip_safe=False,
    dependency_links=[]
)







