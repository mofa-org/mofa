import shutil
from pathlib import Path
from typing import Union
import os
from typing import Optional


def check_dir(dir_path: Union[Path, str]):
    # Convert the input to a Path object if it's not already
    folder_path = Path(dir_path)
    # Return True if the folder exists, otherwise False
    return folder_path.exists() and folder_path.is_dir()


def make_dir(dir_path: Union[Path, str]):
    p: Path = Path(dir_path)
    p.mkdir(exist_ok=True, parents=True)


def remove_dir(dir_path: Union[str, Path]):
    if Path(dir_path).is_dir():
        shutil.rmtree(dir_path)


def delete_all_files_in_folder(folder_path: str):
    """
    Delete all files and subfolders in the specified folder.

    Parameters:
    folder_path (str): Path to the folder whose contents are to be deleted.
    """
    for filename in os.listdir(folder_path):
        file_path = os.path.join(folder_path, filename)
        try:
            if os.path.isfile(file_path) or os.path.islink(file_path):
                os.unlink(file_path)
            elif os.path.isdir(file_path):
                shutil.rmtree(file_path)
        except Exception as e:
            print(f'Failed to delete {file_path}. Reason: {e}')


def get_relative_path(current_file: str, sibling_directory_name: Optional[str] = 'sibling_directory',
                      target_file_name: Optional[str] = 'target_file') -> str:
    """
    Get the relative path to the specified file in a sibling directory.

    Parameters:
    current_file (str): Path to the current script file.
    sibling_directory_name (str, optional): Name of the sibling directory, default is 'sibling_directory'.
    target_file_name (str, optional): Name of the target file, default is 'target_file'.

    Returns:
    str: Relative path to the target file.
    """
    # Directory of the current script
    current_dir = os.path.dirname(current_file)

    # Parent directory of the current directory
    parent_dir = os.path.dirname(current_dir)

    # Relative path to the target file in the sibling directory
    target_file_relative_path = os.path.join(parent_dir, sibling_directory_name, target_file_name)

    return target_file_relative_path


import shutil

def copy_directories(source_directory: str, subdirectories: list, destination_directory: str) -> None:
    """
    Copies all files from specific subdirectories within a source directory to a destination directory,
    maintaining the directory structure.

    Parameters:
    - source_directory: str - The root directory containing the subdirectories to copy.
    - subdirectories: list - A list of subdirectory names to copy.
    - destination_directory: str - The directory to which files and directories will be copied.
    """
    for subdirectory in subdirectories:
        subdirectory_path = os.path.join(source_directory, subdirectory)
        if os.path.exists(subdirectory_path) and os.path.isdir(subdirectory_path):
            # Walk through the subdirectory
            for root, dirs, files in os.walk(subdirectory_path):
                # Construct the corresponding destination path
                relative_path = os.path.relpath(root, source_directory)
                destination_path = os.path.join(destination_directory, relative_path)

                # Create destination directories if they don't exist
                if not os.path.exists(destination_path):
                    os.makedirs(destination_path)

                # Copy each file
                for file in files:
                    source_file_path = os.path.join(root, file)
                    destination_file_path = os.path.join(destination_path, file)
                    shutil.copy2(source_file_path, destination_file_path)
                    print(f"Copied {source_file_path} to {destination_file_path}")
        else:
            print(f"Subdirectory {subdirectory} does not exist in {source_directory}")

def get_subdirectories(directory:str):
    return [p.name for p in Path(directory).iterdir() if p.is_dir()]