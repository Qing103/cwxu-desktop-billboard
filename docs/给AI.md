# 后续实现说明

## 当前产品状态

本仓库是基于 `sapphire-xu/nidong` 桌面壳适配的无锡学院主题桌面看板，属于非官方项目，未获得学校授权或背书。现阶段只提供桌面岛、Fluent 看板、本机日程、周视图、日程提醒和本地设置。

无锡学院的教务登录与课表同步、校园卡、宿舍和校园网接口均未接入。首次使用允许用户创建本机昵称，并选择保存教务账号；用户名只保存在本机 `settings.json`，密码使用独立的系统凭据库条目保存，绝不能写入普通设置、日志或前端持久化。资料页提供删除已保存账号和密码的操作。保存资料本身不发起网络请求；在正式核实无锡学院登录流程、域名校验、认证跳转及会话维护之前，不得自动提交密码或复用东北大学登录代码。当前 Rust 启动路径不会创建历史 NEU 客户端，认证、刷新、账单、课表命令均不可用；自动校园网登录和自动更新已关闭。

## 界面与架构

- 保留顶部 420×40 置顶桌面岛和系统标题栏的 Fluent 桌面窗口。
- 学校数据尚未接入时，显示“待接入”，不要编造余额、课程、宿舍或账号状态。
- 本机日程保存在本机，继续使用现有 agenda 模型和窗口内周视图。
- 只开放已核实的学校官方页面：官网 `https://www.cwxu.edu.cn/`、教务登录页 `https://jwgl.cwxu.edu.cn/jwglxt/xtgl/login_slogin.htm`，以及用户提供的个人课表查询页 `https://jwgl.cwxu.edu.cn/jwglxt/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151`。Tauri `open_door` 只允许精确匹配这些地址。
- 用户确认个人课表请求为 `POST /jwglxt/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151`，表单编码为 `application/x-www-form-urlencoded`；已知参数名为 `gnmkdm`、`xnm`、`xqm`、`kzlx`、`xsdm`、`kclbdm`、`kclxdm`。课程字段名已开始映射（如 `kcmc`、`cdmc`、`jcs`、`xqj`、`zcd`、`xqmc`）；`xm` 的含义不可仅凭字段名当作任课教师。无锡学院官方 1–11 节时间已按用户提供的《学校上课时间表.pdf》写入看板。响应最外层课表列表结构及教务登录/安全会话流程仍待确认，因此自动登录和课表同步继续关闭。不得保存会话 Cookie、令牌或真实参数/课表值，不要复用历史 NEU 端点。
- 主题采用克制的蓝绿色；不要使用未经确认的校徽、官方 VI 或授权声明。

## 主要位置

- `ui/src/App.tsx`：顶部桌面岛
- `ui/src/desk/DeskApp.tsx`：桌面窗口和导航
- `ui/src/desk/pages/`：看板、日程、学校服务、设置和关于
- `ui/src/Settings.tsx`：外观、启动、桌面岛和日程提醒设置
- `src-tauri/src/agenda.rs`：本机日程读写
- `src-tauri/src/lib.rs`：Tauri 状态、命令、窗口和托盘

`docs/开发文档.md` 保留的是上游 NEU 版本的历史记录，不是无锡学院接口资料，不能将其中的域名、流程或参数用于本项目。
