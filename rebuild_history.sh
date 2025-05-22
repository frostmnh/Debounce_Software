#!/bin/bash

# --- 設定 ---
# 要被重建的舊分支名稱
OLD_BRANCH="main"
# 用於重建的、全新的臨時分支名稱
NEW_BRANCH="new-main"

# --- 安全檢查 ---
# 檢查是否有未提交的變更，防止資料遺失
if ! git diff-index --quiet HEAD --; then
    echo "錯誤：您的工作目錄中有未提交的變更。請先 commit 或 stash。"
    exit 1
fi

echo "--- 準備開始重建歷史 ---"
echo "將要重建的分支: ${OLD_BRANCH}"
echo "將要建立的新分支: ${NEW_BRANCH}"
echo "5秒後開始... 按 Ctrl+C 取消。"
sleep 5

# --- 步驟 1: 獲取舊分支的 commit 列表 (從最舊到最新) ---
# git rev-list 會列出所有可達的 commit hash
# --reverse 參數讓輸出從最舊的 commit 開始
COMMIT_LIST=$(git rev-list --reverse ${OLD_BRANCH})

# --- 步驟 2: 建立一個全新的孤兒分支 ---
git checkout --orphan ${NEW_BRANCH}
# 清理暫存區，為第一次 commit 做準備
git rm -rf .

echo "--- 已建立新的孤兒分支: ${NEW_BRANCH} ---"

# --- 步驟 3: 循環遍歷並重建每一個 commit ---
for COMMIT_HASH in ${COMMIT_LIST}
do
    echo "正在處理舊 commit: ${COMMIT_HASH}"

    # a. 從舊 commit 中提取所有檔案，覆蓋目前工作目錄
    #    `-- .` 表示提取所有檔案
    git checkout ${COMMIT_HASH} -- .

    # b. 提取舊 commit 的元數據 (作者、Email、時間、訊息)
    AUTHOR_NAME=$(git show -s --format='%an' ${COMMIT_HASH})
    AUTHOR_EMAIL=$(git show -s --format='%ae' ${COMMIT_HASH})
    AUTHOR_DATE=$(git show -s --format='%ad' ${COMMIT_HASH})
    COMMIT_MESSAGE=$(git show -s --format=%B ${COMMIT_HASH})

    # c. 將所有檔案加入暫存区
    git add .

    # d. 建立一個全新的、帶 GPG 簽名的 commit，並「嫁接」舊的元數據
    #    我們使用環境變數來臨時覆寫作者和時間，使其與原始 commit 保持一致
    GIT_AUTHOR_NAME="${AUTHOR_NAME}" \
    GIT_AUTHOR_EMAIL="${AUTHOR_EMAIL}" \
    GIT_AUTHOR_DATE="${AUTHOR_DATE}" \
    git commit -S --message="${COMMIT_MESSAGE}"
    
    # GPG 密碼框可能會在這裡彈出 (通常只在第一次時)
done

echo "--- 所有 commit 已成功重建並簽名！ ---"

# --- 步驟 4: (可選但建議) 驗證新分支 ---
echo "--- 正在驗證新分支 ${NEW_BRANCH} 的簽名狀態 ---"
git log --graph --pretty="format:%h %G? %GS %s"

# --- 步驟 5: 用新分支替換舊分支 ---
echo "--- 準備用 ${NEW_BRANCH} 替換 ${OLD_BRANCH} ---"
# 切換到一個無關的分支，以安全地刪除舊的 main 分支
# 我們可以臨時切換到新分支的某個 commit 上 (分離 HEAD 狀態)
git checkout $(git rev-parse ${NEW_BRANCH})

# 刪除舊的 main 分支
git branch -D ${OLD_BRANCH}

# 將新分支重命名為 main
git branch -m ${NEW_BRANCH} ${OLD_BRANCH}

# 切換回新的 main 分支
git checkout ${OLD_BRANCH}

echo "--- 歷史重建完成！現在位於全新的、已簽名的 '${OLD_BRANCH}' 分支上。---"
echo "下一步，請手動執行 'git remote add origin ...' 和 'git push --force origin main'"
