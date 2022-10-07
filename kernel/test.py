
import re
from tokenize import Double
 
 
fname = 'result.txt'
with open(fname) as f:
    s = f.read()
numbers = [float(i) for i in re.findall(r"\d+\.?\d*", s)]
print(f'numbers: {numbers}\navg: {sum(numbers) / len(numbers)}')