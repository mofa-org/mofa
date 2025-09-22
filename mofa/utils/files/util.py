import os
from pathlib import Path
from typing import Union, Generator, List
from typing import Optional

def get_file_name(file_path:str):
    return Path(file_path).name

def get_all_files(dir_path: Union[str, Path], file_type: Union[str, None] = None) -> Generator:
    for root, ds, fs in os.walk(dir_path):
        for f in fs:
            fullname = os.path.join(root, f)
            if file_type is not None:
                if isinstance(file_type, str):
                    file_type = [file_type]
            if '.' == Path(fullname).name[0] or (
                file_type is not None and (Path(fullname).suffix.replace('.', '') not in file_type)):
                continue

            yield fullname

def create_file_dir(file_path:str):
    db_dir = os.path.dirname(file_path)
    if not os.path.exists(db_dir):
        os.makedirs(db_dir)


def find_file(target_filename: str, search_directory: str) -> Optional[str]:
    """
    Recursively searches for a file in the specified directory and returns its absolute path.

    Parameters:
    - target_filename: str - The name of the file to search for.
    - search_directory: str - The directory in which to start the search.

    Returns:
    - Optional[str] - The absolute path of the found file, or None if not found.
    """
    # Walk through the directory
    for root, dirs, files in os.walk(search_directory):
        if target_filename in files:
            # Construct the full file path and return it
            return os.path.abspath(os.path.join(root, target_filename))

    # Return None if the file is not found
    return None

def get_files_in_directory(directory: str) -> List[str]:

    return [os.path.join(directory, file) for file in os.listdir(directory) if os.path.isfile(os.path.join(directory, file))]

























