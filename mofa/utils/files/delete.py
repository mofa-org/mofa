import os

def delete_file(file_path: str) -> None:
    """
    Delete the specified file.

    :param file_path: The path of the file to be deleted
    """
    if os.path.exists(file_path):
        os.remove(file_path)
        print(f"The file '{file_path}' has been deleted.")
    else:
        print(f"The file '{file_path}' does not exist.")