base = 28
total = 0

for i in range(0, 28):  # 1 to 27
    exponent = 27 - i
    total += i * (base**exponent)

print(total)
