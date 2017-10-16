import sqlite3
import json

# Creates or opens a file called mydb with a SQLite3 DB
#db = sqlite3.connect('/tmp/datalog_queue.db')
db = sqlite3.connect('/tmp/datalog_queue.db', check_same_thread=False)

# Get a cursor object
cursor = db.cursor()

cursor.execute("CREATE TABLE IF NOT EXISTS readings(id INTEGER PRIMARY KEY, reading TEXT);")
db.commit()

rdg = json.dumps({'a': 'b', 'c': {'d': 'e'}})
cursor.execute("INSERT INTO readings(reading) VALUES(?)", (rdg,))


cursor.execute("SELECT * FROM readings ORDER BY id DESC LIMIT 1;")

lastrow = cursor.fetchone()
if lastrow:
	(lastid, reading) = lastrow


cursor.execute("DELETE FROM readings WHERE id = (?);", (lastid,))
db.commit()

