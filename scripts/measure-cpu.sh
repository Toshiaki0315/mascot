#!/bin/zsh
# アプリの CPU 使用量を測る。
# 使い方: scripts/measure-cpu.sh <アプリの実行ファイル> [秒数（既定 60）]
#   アプリ本体と、起動後に増えた画面描画のプロセス（WebContent）の CPU 時間を測り、
#   測った秒数あたりの使用率（%）を出す。マスコットは入力が 60 秒ないと眠って軽くなるので、
#   最後の「入力が途切れていた秒数」も見て、起きていたか眠っていたかを見分ける。
#   起動できるのは 1 つだけなので、普段のアプリと同じ識別子のものは、普段のアプリを止めてから測る
set -u
BIN=$1
SECS=${2:-60}

before=$(pgrep -f com.apple.WebKit.WebContent | sort)
"$BIN" > /dev/null 2>&1 &
APP=$!
sleep 8
after=$(pgrep -f com.apple.WebKit.WebContent | sort)
WEB=$(comm -13 <(echo "$before") <(echo "$after") | tr '\n' ' ')

# ps の cputime（[時:]分:秒.百分の一秒）を秒に直す
cpu() { ps -o cputime= -p "$1" | awk -F'[:.]' '{ if (NF == 4) print $1*3600 + $2*60 + $3 + $4/100; else print $1*60 + $2 + $3/100 }'; }
total() { local sum=0; for p in ${=1}; do sum=$(echo "$sum + $(cpu $p)" | bc); done; echo $sum; }

app0=$(cpu $APP); web0=$(total "$WEB")
sleep $SECS
app1=$(cpu $APP); web1=$(total "$WEB")
idle=$(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ { print int($NF / 1e9); exit }')
kill $APP

printf "app: %s%%  webcontent: %s%%  (入力が途切れていた秒数: %ss)\n" \
  "$(echo "scale=2; ($app1 - $app0) * 100 / $SECS" | bc)" \
  "$(echo "scale=2; ($web1 - $web0) * 100 / $SECS" | bc)" \
  "$idle"
