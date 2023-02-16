import matplotlib.pyplot as plt
import numpy as np

ft = open("futuresdr_dump.f32", "rb")
data = np.fromfile(ft, dtype=np.float32, count=100000, sep='')

print(data[0:10000])
fig = plt.figure()
plt.plot(data)

plt.show()