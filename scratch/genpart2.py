def print_partition(ans):
    for subset in ans:
        print("{", end=" ")
        for element in subset:
            print(element, end=" ")
        print("}", end=" ")
    print()


def partition_string(s, index, ans):
    if index == len(s):
        print_partition(ans)
        return

    for i in range(len(ans)):
        ans[i].append(s[index])
        partition_string(s, index + 1, ans)
        ans[i].pop()

    ans.append([s[index]])
    partition_string(s, index + 1, ans)
    ans.pop()


def all_partitions_of_string(s):
    ans = []
    partition_string(s, 0, ans)


if __name__ == "__main__":
    s = "ABCD"
    print("All partitions of the string will be:")
    all_partitions_of_string(s)