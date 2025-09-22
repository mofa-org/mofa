import os
import re
from pathlib import Path
from typing import List

import pandas as pd
import yaml

from mofa.utils.files.write import ensure_directory_exists


def read_yaml(file_path:str):
    with open(file_path, 'r') as file:
        prime_service = yaml.safe_load(file)
    return prime_service

def read_text(file_path: str = '/mnt/d/project/dy/extra/nlp/uie/The Three-Body Problem 1: The Madness Years.txt',encoding:str='utf-8',is_loda_lines:bool=False):
    with open(file_path, 'r', encoding=encoding) as f:
        if is_loda_lines is True:
            lines = f.readlines()
            return lines
        else:
            content = ""
            content = f.read()
            return content




def read_excel(file_path:str, sheet_names:list[str]=None)->[dict]:
    """
    Read all sheets or specified sheets from an Excel file and generate a list of dictionaries.

    Parameters:
    file_path (str): Path to the Excel file.
    sheet_names (list, optional): List of sheet names to read. If not provided, all sheets will be read.

    Returns:
    list: A list of dictionaries, each containing the data from a sheet. Each dictionary's key is the sheet name, and the value is the content (DataFrame) of that sheet.
    """

    xls = pd.ExcelFile(file_path)
    all_sheet_names = xls.sheet_names

    if sheet_names is None:
        sheet_names = all_sheet_names

    invalid_sheets = [sheet for sheet in sheet_names if sheet not in all_sheet_names]
    if invalid_sheets:
        raise ValueError(f"The following sheets are not found in the Excel file: {', '.join(invalid_sheets)}")
    sheets_data = []
    for sheet_name in sheet_names:
        sheet_df = pd.read_excel(file_path, sheet_name=sheet_name)
        sheets_data.append({sheet_name: sheet_df})

    return sheets_data


def modify_agents_inputs(file_path: str, new_inputs: List[str], output_file_path: str) -> None:
    """
    Modify the agents_inputs list in a Python file by adding new inputs.

    This function reads a Python file, finds the agents_inputs list,
    adds new inputs to the list while keeping the original ones,
    and writes the modified content to a new file.


    Additionally, it adds new input handling logic at the specified location.

    :param file_path: Path to the Python file to be modified.

    :param new_inputs: List of new input strings to add to the agents_inputs list.
    :param output_file_path: Path to the new file where modified content will be saved.
    :raises ValueError: If the agents_inputs list is not found in the file.
    """
    with open(file_path, 'r') as file:
        code = file.read()

    # Use regex to find the agents_inputs list and extract existing content
    pattern = r"(agent_inputs\s*=\s*\[)([^\]]*)(\])"
    match = re.search(pattern, code)

    if not match:
        raise ValueError("agent_inputs list not found")

    # Extract existing agents_inputs list content
    existing_inputs_str = match.group(2)
    existing_inputs = [input_.strip().strip("'").strip('"') for input_ in existing_inputs_str.split(',') if input_.strip()]

    # Add new inputs to the existing inputs and remove duplicates
    updated_inputs = list(set(existing_inputs + new_inputs))

    # Construct new agents_inputs list string
    new_inputs_str = ', '.join([f"'{input_}'" for input_ in updated_inputs])
    modified_code = re.sub(pattern, rf"\1{new_inputs_str}\3", code)

    # Define the pattern to find the place where to insert the new handling logic
    insertion_point_pattern = r"(\s+result\s*=\s*run_dspy_agent\(inputs=inputs\))"
    insertion_match = re.search(insertion_point_pattern, modified_code)

    if not insertion_match:
        raise ValueError("Insertion point for handling logic not found")

    # Extract the current indentation level from the insertion point
    indentation = insertion_match.group(1)[:insertion_match.group(1).index('result')]

    # Create the new handling logic string with proper indentation
    handling_logic_lines = [
        "# Handling new agent inputs",
    ]
    handling_logic_lines.extend([f"inputs['input_fields']['{input_}'] = dora_result.get('{input_}')"
                                 for input_ in new_inputs])
    handling_logic = "\n".join([indentation + line for line in handling_logic_lines])

    # Check if 'inputs['input_fields']' exists
    input_fields_pattern = r"(inputs\['input_fields'\] = \{[^\}]*\})"
    input_fields_match = re.search(input_fields_pattern, modified_code)

    if input_fields_match:
        # If exists, add new input fields after its definition
        input_fields_code = input_fields_match.group(1)
        new_fields_code = ", ".join([f"'{input_}': dora_result.get('{input_}')"
                                     for input_ in new_inputs])
        # Insert the new fields into the existing 'inputs['input_fields']' dictionary
        updated_input_fields_code = re.sub(r"\}", r", " + new_fields_code + r"}", input_fields_code)
        modified_code = modified_code.replace(input_fields_code, updated_input_fields_code)
    else:
        # If not exists, create 'inputs['input_fields']' and add new input fields
        create_input_fields = f"{indentation}inputs['input_fields'] = {{}}\n{handling_logic}\n"
        modified_code = re.sub(insertion_point_pattern, create_input_fields + r"\1", modified_code)
    ensure_directory_exists(file_path=output_file_path)
    # Write the modified content to the new file
    with open(output_file_path, 'w') as file:
        file.write(modified_code)


def read_file_content(file_path:str):
    file_path = Path(file_path)
    if file_path.is_file():
        with file_path.open('r', encoding='utf-8') as file:
            return file.read()
    else:
        return file_path
    
def flatten_dict_simple(nested_dict:dict, parent_key="", sep="."):
    flat_dict = {}
    for key, value in nested_dict.items():
        if isinstance(value, dict):
            flat_dict.update(flatten_dict_simple(value, parent_key, sep))
        else:
            flat_dict[key] = value
    return flat_dict


