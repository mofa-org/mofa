import json
import time
import pandas as pd
import os
import yaml
def get_log(dbname="logs.db", table="chat_completions"):
    import sqlite3

    con = sqlite3.connect(dbname)
    query = f"SELECT * from {table}"
    cursor = con.execute(query)
    rows = cursor.fetchall()
    column_names = [description[0] for description in cursor.description]
    data = [dict(zip(column_names, row)) for row in rows]
    con.close()
    return data
def str_to_dict(s):
    return json.loads(s)
def get_local_timestamp():
    return int(time.time())

def create_sqlite_db_name():
    return str(get_local_timestamp()) + '_logs.db'
def remove_sqlite_db_file(dbname="logs.db"):
    if os.path.exists(dbname):
        # 删除文件
        os.remove(dbname)


def load_log_token(logging_session_id:int,dbname="logs.db", table="chat_completions"):
    log_data = get_log(dbname=dbname,table=table)
    log_data_df = pd.DataFrame(log_data)

    log_data_df["total_tokens"] = log_data_df.apply(
        lambda row: str_to_dict(row["response"])["usage"]["total_tokens"], axis=1
    )

    log_data_df["request"] = log_data_df.apply(lambda row: str_to_dict(row["request"])["messages"][0]["content"], axis=1)

    log_data_df["response"] = log_data_df.apply(
        lambda row: str_to_dict(row["response"])["choices"][0]["message"]["content"], axis=1
    )

    total_tokens = log_data_df["total_tokens"].sum()

    # Sum total cost for all sessions
    total_cost = log_data_df["cost"].sum()

    session_tokens = log_data_df[log_data_df["session_id"] == logging_session_id]["total_tokens"].sum()
    session_cost = log_data_df[log_data_df["session_id"] == logging_session_id]["cost"].sum()

    return {'total_tokens':str(total_tokens),'total_cost': str(round(total_cost, 4)),
            'session_id':logging_session_id,}



