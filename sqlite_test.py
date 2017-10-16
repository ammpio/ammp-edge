import sqlite3

# Creates or opens a file called mydb with a SQLite3 DB
db = sqlite3.connect('/tmp/datalog_queue.db')

# Get a cursor object
cursor = db.cursor()

cursor.execute("CREATE TABLE IF NOT EXISTS readings(id INTEGER PRIMARY KEY, reading TEXT);")
db.commit()




db.commit()

