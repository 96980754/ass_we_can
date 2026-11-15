# 全球提肛排行榜 · 设计方案（v1）

> 目标：在**不破坏本地优先、隐私优先**的前提下，让用户可选地加入全球排行榜。
> 客户端默认完全离线；只有用户显式开启并填写服务器地址后，才会上传**每日聚合数据**。

---

## 1. 设计原则

| 原则 | 落地方式 |
| --- | --- |
| 本地优先 | 无网络也能完整使用；同步失败不阻塞任何功能 |
| 隐私最小化 | 只上传「日期 + 次数 + 总时长 + 时区 + 昵称」，不上传单次时间戳、不上传设备指纹 |
| 匿名身份 | 安装时本地生成随机 `device_id`，注册换 token；不收集手机号/邮箱 |
| 显式选择 | 排行榜开关默认关闭，开启时展示「将上传哪些字段」 |
| 可退出 | 支持一键删除服务端数据（`DELETE /v1/me`） |
| 可验证 | 服务端做合理性校验 + 可选请求签名，异常数据进影子榜 |

---

## 2. 总体架构

```
┌──────────────┐   HTTPS    ┌──────────────┐     ┌──────────────┐
│ Tauri 客户端  │ ─────────► │  API 网关     │ ──► │  PostgreSQL  │
│ (Rust reqwest)│ ◄───────── │ (Go/Node)    │ ──► │  Redis ZSET  │
└──────────────┘            └──────────────┘     └──────────────┘
       │ 本地 outbox 队列            │ 限流 / WAF
       ▼                            ▼
  离线可重试                     CDN / 缓存
```

- **PostgreSQL**：设备、日汇总、昵称、封禁/可疑标记。
- **Redis ZSET**：`lb:{metric}:{period}:{scope}`，天然支持 Top N 与「我的排名」。
- **API 网关**：限流（每设备每分钟 60 次）、请求体大小限制、可选 HMAC/Ed25519 验签。
- **CDN**：排行榜读接口 `Cache-Control: public, max-age=60`，写接口不缓存。

MVP 可以直接用 Supabase / Cloudflare Workers + D1 + KV，省掉运维。

---

## 3. 身份与注册

安装后首次开启排行榜：

```http
POST /v1/devices
Content-Type: application/json

{ "platform": "windows", "appVersion": "0.2.0" }
```

```json
{ "deviceId": "dev_01J...", "token": "32字节随机串", "nickname": "匿名蜜桃#4821" }
```

- `device_id`：ULID，客户端生成或服务端生成。
- `token`：仅哈希入库（Argon2id），客户端存于本地 secure storage。
- 昵称：用户可改，1–16 字符，服务端过滤敏感词与重名。

---

## 4. 数据模型

```sql
-- 设备
CREATE TABLE devices (
  id            TEXT PRIMARY KEY,          -- dev_...
  nickname      TEXT NOT NULL,
  token_hash    TEXT NOT NULL,
  platform      TEXT NOT NULL,
  app_version   TEXT NOT NULL,
  suspicious    BOOLEAN NOT NULL DEFAULT FALSE,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 每日聚合（幂等：同设备同一天只保留一条）
CREATE TABLE daily_stats (
  device_id     TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  local_date    DATE NOT NULL,
  tz_offset_min INT  NOT NULL,
  reps          INT  NOT NULL CHECK (reps >= 0),
  total_ms      BIGINT NOT NULL CHECK (total_ms >= 0),
  sessions      INT  NOT NULL DEFAULT 0,   -- 完成的「组」数
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (device_id, local_date)
);

CREATE INDEX idx_daily_date ON daily_stats (local_date);
```

**周期榜查询方式**

| 榜单 | 计算 |
| --- | --- |
| 今日 | `local_date = today` 按 `reps` 求和 |
| 本周 | 按用户时区划分周一至今，`SUM(reps)` |
| 总榜 | `SUM(reps)` 全量 |
| 时长榜 | 同上，按 `total_ms` |

> 时区处理：以用户上报的 `local_date` 为准（客户端写入时固化），服务端不做二次换算，避免跨时区错位。

---

## 5. API 契约

### 5.1 上报日汇总（幂等）

```http
PUT /v1/stats/{localDate}          # 例：/v1/stats/2026-11-15
Authorization: Bearer <token>
Idempotency-Key: <deviceId>:<localDate>

{
  "reps": 42,
  "totalMs": 215000,
  "sessions": 4,
  "tzOffsetMin": 480,
  "appVersion": "0.2.0"
}
```

- 同一 `localDate` 重复上报 = 覆盖（以客户端最终值为准）。
- 服务端校验：`reps <= totalMs / 1000`、`reps <= 2000/天`、`totalMs <= 6h/天`；越界直接 422 并标记 `suspicious`。
- 客户端在「每组完成」「每天首次启动」「退出前」各尝试一次；失败进入 outbox。

### 5.2 排行榜

```http
GET /v1/leaderboard?period=daily|weekly|monthly|all&metric=reps|minutes&limit=50&offset=0
```

```json
{
  "period": "weekly",
  "metric": "reps",
  "updatedAt": "2026-11-15T10:00:00Z",
  "entries": [
    { "rank": 1, "nickname": "匿名蜜桃#4821", "value": 1280, "platform": "macos" },
    { "rank": 2, "nickname": "坚持就是胜利", "value": 1175, "platform": "windows" }
  ]
}
```

### 5.3 我的排名

```http
GET /v1/me/rank?period=weekly&metric=reps
Authorization: Bearer <token>
→ { "rank": 1832, "total": 50213, "value": 128, "percentile": 0.963 }
```

### 5.4 删除数据

```http
DELETE /v1/me
Authorization: Bearer <token>
→ 204   # 级联删除设备与所有日汇总
```

---

## 6. 反作弊

1. **物理上限**：单次 ≤ 10 分钟、单日次数 ≤ 2000、单日总时长 ≤ 6 小时、日活跃时长 ≤ 24 小时。
2. **一致性**：`reps * 1000 <= totalMs`（每次至少 1 秒）。
3. **增速检测**：与设备近 7 日均值比较，超过 5 倍标记 `suspicious`；连续 3 天进入影子榜。
4. **功率谱/分布检测（进阶）**：真实用户的单次时长分布长尾明显；脚本刷量通常集中在阈值附近。
5. **可选签名**：客户端生成 Ed25519 密钥对，注册时上传公钥；上报体带时间戳签名，拒绝重放（±5 分钟）。
6. **影子榜**：可疑数据仍入库、仍计入个人统计，但不进入公开榜单，避免误伤正常用户。

---

## 7. 客户端改造建议

| 模块 | 内容 |
| --- | --- |
| 设置项 | `leaderboardEnabled`（默认 false）、`serverUrl`、`nickname`、`deviceToken` |
| 本地队列 | `outbox.jsonl`：`{localDate, reps, totalMs, sessions, idemKey}`，成功即删 |
| 触发点 | 每组完成、每日首次启动、退出前、每 5 分钟重试 |
| 网络层 | 建议放在 **Rust 侧**（`reqwest`），避免放宽 WebView CSP；超时 8s，指数退避 |
| 界面 | 面板新增「排行榜」页签：今日/本周/总榜分段控件 + Top 50 + 「我的排名/百分位」 |
| 隐私 | 首次开启弹确认，列出上传字段；提供「清除服务端数据」按钮 |
| 降级 | 未配置服务器 / 请求失败时显示「离线模式，仅本地统计」 |

> 出于减少攻击面考虑，**不要**在前端 `fetch` 里暴露 token；用 Tauri 命令 `sync_stats` / `fetch_leaderboard` 代发。

---

## 8. 里程碑

- **M1（1–2 天）**：匿名注册 + 日汇总上报 + 全球总榜（次数）。
- **M2（2–3 天）**：周榜/月榜、时长榜、我的排名与百分位、Redis 缓存。
- **M3（3–5 天）**：反作弊（上限校验 + 增速标记 + 影子榜）、Ed25519 签名、删除数据。
- **M4**：地区榜 / 好友榜 / 周报推送。

---

## 9. 待确认

1. 你的服务器技术栈（Go / Node / Python / Supabase）？
2. 排行榜是否需要昵称唯一、是否需要 OAuth 登录（否则纯匿名）？
3. 榜单口径：按「次数」还是「总时长」，或两者都做？
4. 是否需要「好友/团队」榜（需要邀请码，会引入额外数据模型）？

确认后即可按第 5 节契约实现服务端与客户端 `sync` 模块。
