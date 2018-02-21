
import minimalmodbus, serial

try:
    c = minimalmodbus.Instrument(port='/dev/ttyS0', slaveaddress=1)
except:
    print('uh oh')

print ('wooo')
