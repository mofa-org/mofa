import uuid
import time
def get_variable_name(variable, local_vars):
    return [name for name, value in local_vars.items() if value is variable]


def clean_string(input_string:str):
    return input_string.encode('utf-8', 'replace').decode('utf-8')


def generate_unique_int():
    timestamp_str = str(time.time()).replace('.', '')[:8]  # 取前8个字符
    unique_part = str(int(uuid.uuid4().hex[:8], 16))[:8]  # 取前8个字符
    unique_id_str = timestamp_str + unique_part
    unique_id_int = int(unique_id_str)

    return unique_id_int



def while_input(input_hint:str):
    while True:
        input_data = input(
        input_hint,
        )
        if input_data == "" or input_data == ' ':
            continue
        else:
            return input_data
