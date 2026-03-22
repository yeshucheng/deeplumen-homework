注意：通过 [patch.crates-io] 指向 fork 的 spider 分支，
将其内部 reqwest 切到 native-tls，绕过 Bazel 下 aws-lc-sys 构建失败问题。
1.拷贝.env.example作为.env，按照里面对应的填写内容

2.把run.sh修改可执行文件
./run.sh

3.可以安装客户端sqllite查看对应数据库脚本

4.bazel run //:my_spider  即可得到效果数据获取存入sqllite中
