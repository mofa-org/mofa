import os
from dotenv import load_dotenv, dotenv_values

env_file = '.env.test'
with open(env_file, 'w') as f:
    f.write('KEY=val1\nKEY=val2\n')

# Test 1: load_dotenv with override=True
load_dotenv(env_file, override=True)
print(f'os.getenv (last wins): {os.getenv("KEY")}')

# Test 2: dotenv_values
values = dotenv_values(env_file)
print(f'dotenv_values (last wins): {values["KEY"]}')
