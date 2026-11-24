while read -t "$1" _ || [ "$?" -gt 128 ]; do
    echo || exit
done
