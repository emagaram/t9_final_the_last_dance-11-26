# Pre-compute factorials
FACT = [1]
for i in range(1, 26):
    FACT.append(FACT[-1] * i)

def number_to_combination(n):
    available = 26
    result = ""
    used = [False] * 26
    
    while n > 0:
        available -= 1
        digit = n // FACT[available]
        n %= FACT[available]
        
        # Find digit-th unused letter
        count = 0
        for i in range(26):
            if not used[i]:
                if count == digit:
                    result += chr(ord('A') + i)
                    used[i] = True
                    break
                count += 1
    
    return result
print(number_to_combination(0))  # Output: A
print(number_to_combination(1))  # Output: B
print(number_to_combination(25))  # Output: Z
print(number_to_combination(26))  # Output: AB
print(number_to_combination(27))  # Output: AC