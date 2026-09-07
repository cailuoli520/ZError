<template>
  <div class="home-page">
    <div class="server-control">
      <div class="server-card">
        <div class="server-main-content">
          <div class="server-info">
            <div class="server-icon" :class="{ 'running': serverRunning, 'stopped': !serverRunning }">
            </div>
            <div class="server-details">
              <p class="server-url">
                <template v-if="serverRunning">
                  <span class="server-status-text">运行中</span>
                  <span v-if="serverStatus?.version" class="server-meta">v{{ serverStatus.version }}</span>
                  <span v-if="serverStatus" class="server-meta">已运行 {{ formatUptime(serverStatus.uptime_secs) }}</span>
                  <span class="server-meta">最近 OCS 探测: {{ formatOcsContact(serverStatus?.last_ocs_contact_at) }}</span>
                </template>
                <span v-else class="url-placeholder">{{ statusError || '正在获取服务状态...' }}</span>
              </p>
            </div>
          </div>
          <div class="server-config-controls">
            <div class="tooltip-container">
              <button class="config-btn ocs-config" @click="openOCSConfig">
                <svg width="800" height="800" viewBox="0 0 800 800" shape-rendering="geometricPrecision"
                  xmlns="http://www.w3.org/2000/svg" fill="currentColor">
                  <g stroke="null">
                    <g stroke="null">
                      <path stroke="null" opacity="1"
                        d="m382.29066,36c12.1533,0 24.30713,0 36.46043,0c90.68231,5.71443 168.8118,39.84485 234.3885,102.39137c6.51079,6.54676 13.02158,13.09353 19.53237,19.64029c0.3021,0.7479 0.21355,1.44657 -0.26043,2.09496c-43.79419,43.2521 -87.37482,86.81007 -130.7367,130.67338c34.98118,48.09304 44.96613,101.07835 29.94964,158.9554c-2.51056,8.36991 -5.46386,16.57483 -8.85468,24.61583c-11.80641,-11.87164 -23.6123,-23.74275 -35.41871,-35.61439c13.18878,-52.67211 0.77504,-98.15064 -37.24173,-136.43453c-36.96359,-31.76751 -79.15352,-41.71859 -126.56979,-29.85324c-41.61073,13.20927 -70.51864,40.18193 -86.72374,80.91799c-6.98269,18.77193 -9.58701,38.15034 -7.81295,58.13525c1.00631,9.21627 2.5689,18.29427 4.68777,27.23453c-11.63973,11.79151 -23.35916,23.48821 -35.15827,35.09065c-21.92522,-50.63109 -21.49134,-101.08463 1.30216,-151.36115c20.91423,-42.50368 53.29421,-72.44438 97.14101,-89.82158c50.90345,-17.98841 100.38547,-14.67103 148.44605,9.95108c8.21037,4.69377 16.1103,9.84371 23.69928,15.45036c32.46697,-32.64635 64.93603,-65.29322 97.40144,-97.93957c0.34898,-0.34934 0.34898,-0.69815 0,-1.04748c-57.13558,-48.60893 -123.37221,-74.18514 -198.70936,-76.72806c-93.66529,-0.69134 -172.9235,32.56622 -237.77411,99.77266c-68.72219,76.44733 -95.98068,166.00705 -81.77554,268.67914c16.97441,95.0548 65.67514,168.46653 146.10216,220.2331c79.79209,47.47712 164.51937,58.99942 254.1813,34.56691c44.38224,-13.2297 83.96838,-34.87591 118.75684,-64.94389c-10.32872,-10.38578 -20.66265,-20.77681 -30.99137,-31.16259c-59.52895,49.04311 -127.84903,70.08178 -204.95972,63.11079c-60.75662,-6.95528 -113.36382,-31.2202 -157.82159,-72.8c-7.38636,-7.33761 -14.67845,-14.75902 -21.87626,-22.25899c32.81439,-33.34502 65.80223,-66.51511 98.96403,-99.51079c8.08119,8.03838 16.24104,15.98196 24.48058,23.83022c36.73076,29.45415 77.87896,38.00893 123.44461,25.66331c28.33497,-8.68886 51.16649,-25.0123 68.49353,-48.96978c0.55316,-0.73271 1.24747,-1.25645 2.08345,-1.57122c11.09803,10.54814 21.94918,21.37178 32.55396,32.47194c-39.25226,48.79171 -90.29687,71.92535 -153.13382,69.39568c-35.57705,-2.66584 -67.52316,-14.71189 -95.83885,-36.13813c-0.52086,-0.34934 -1.04173,-0.34934 -1.56259,0c-11.28555,11.3479 -22.57057,22.6937 -33.85612,34.04317c57.26267,45.04173 121.67576,60.23022 193.24029,45.56547c35.89634,-8.22797 67.84245,-24.11304 95.83885,-47.66043c10.48029,-9.57922 20.72515,-19.44127 30.73094,-29.59137c31.50702,31.41922 62.93071,62.92748 94.27626,94.53525c0.30731,0.24092 0.65629,0.32996 1.04173,0.26187c0.47399,0.64944 0.56253,1.34601 0.26043,2.09496c-66.483,73.18233 -149.03775,113.8613 -247.67051,122.03166c-16.32021,0 -32.64094,0 -48.96115,0c-106.75354,-9.01358 -193.65074,-55.10279 -260.6921,-138.26763c-48.46769,-63.12127 -74.25041,-134.70095 -77.3482,-214.73381c0,-7.15797 0,-14.31541 0,-21.47338c3.87934,-92.60789 36.95416,-172.74026 99.22446,-240.39712c66.97677,-69.32278 148.66585,-107.03213 245.0662,-113.12806z"
                        fill="currentColor" />
                    </g>
                    <g stroke="null">
                      <path stroke="null" opacity="1"
                        d="m696.37125,191.02734c0.79171,0.00576 1.39591,0.35457 1.82302,1.04748c38.54909,56.84056 59.81594,119.68949 63.80576,188.54676c0,12.91912 0,25.83772 0,38.75683c-4.00544,69.127 -25.44417,132.15086 -64.32662,189.07051c-0.43753,0.78561 -0.86984,0.95845 -1.30216,0.52374c-10.92771,-11.16092 -21.86584,-22.33755 -32.81439,-33.51942c25.06915,-38.32999 41.29925,-80.22927 48.70072,-125.69784c11.92777,-81.28827 -4.30754,-156.27018 -48.70072,-224.94676c11.24544,-11.04308 22.18357,-22.30351 32.81439,-33.7813z"
                        fill="currentColor" />
                    </g>
                  </g>
                </svg>
              </button>
              <span class="tooltip">OCS题库配置</span>
            </div>
            <div class="tooltip-container">
              <button class="config-btn model-config" @click="openModelSelector">
                <svg width="20" height="20" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M469.333333 42.666667v42.666666H298.666667a128 128 0 0 0-128 128v128a213.333333 213.333333 0 0 0 213.333333 213.333334h256a213.333333 213.333333 0 0 0 213.333333-213.333334V213.333333a128 128 0 0 0-128-128h-170.666666V42.666667h-85.333334zM256 213.333333a42.666667 42.666667 0 0 1 42.666667-42.666666h426.666666a42.666667 42.666667 0 0 1 42.666667 42.666666v128a128 128 0 0 1-128 128H384a128 128 0 0 1-128-128V213.333333z m149.333333 170.666667a64 64 0 1 0 0-128 64 64 0 0 0 0 128z m213.333334 0a64 64 0 1 0 0-128 64 64 0 0 0 0 128zM256 938.666667a256 256 0 0 1 512 0h85.333333a341.333333 341.333333 0 1 0-682.666666 0h85.333333z"
                    fill="#718096" />
                </svg>
              </button>
              <span class="tooltip">选择模型</span>
            </div>
            <div class="tooltip-container">
              <button class="config-btn folder-config" @click="openFolderPicker">
                <svg width="20" height="20" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
                  <path
                    d="M418.688 133.333333a122.666667 122.666667 0 0 1 93.013333 42.666667H785.066667a186.666667 186.666667 0 0 1 186.581333 181.162667l0.085333 5.504v17.92h-0.256c0.170667 1.429333 0.256 2.88 0.256 4.373333V789.333333a186.666667 186.666667 0 0 1-186.666666 186.666667H238.933333A186.666667 186.666667 0 0 1 52.266667 789.333333V320a186.666667 186.666667 0 0 1 186.666666-186.666667z m0 74.666667H238.933333A112 112 0 0 0 126.933333 320v469.333333c0 61.866667 50.133333 112 112 112h546.133334c61.866667 0 112-50.133333 112-112V422.272H615.04a122.666667 122.666667 0 0 1-113.834667-76.992l-1.834666-4.842667-35.413334-100.416a48 48 0 0 0-45.269333-32.021333zM448 688a37.333333 37.333333 0 0 1 3.072 74.538667L448 762.666667h-170.666667a37.333333 37.333333 0 0 1-3.072-74.538667L277.333333 688h170.666667z m337.066667-437.333333H546.88l22.912 64.917333a48 48 0 0 0 41.685333 31.914667l3.562667 0.128 281.024-0.021334a112.021333 112.021333 0 0 0-106.389333-96.853333l-4.608-0.085333z"
                    fill="currentColor" />
                </svg>
              </button>
              <span class="tooltip">选择保存文件夹</span>
            </div>
          </div>
        </div>
        <div class="server-action">
          <form class="test-query-form" @submit.prevent="runTestQuery">
            <input v-model="testQueryTitle" class="test-query-input" type="text" placeholder="输入题目进行测试查询"
              :disabled="testQueryRunning" />
            <button type="submit" class="test-query-btn" :class="{ loading: testQueryRunning }"
              :disabled="testQueryRunning || !testQueryTitle.trim()">
              {{ testQueryRunning ? '查询中' : '测试查询' }}
            </button>
          </form>
        </div>
      </div>
      <div v-if="testQueryResult" class="test-query-result" :class="{ error: testQueryError }">
        <pre>{{ testQueryResult }}</pre>
      </div>
    </div>

    <!-- 请求记录区域 -->
    <div class="request-logs-layout">
      <!-- 请求记录列表 -->
      <div class="request-logs-main">
        <div class="request-logs-card">
          <div class="request-logs-header">
            <div class="request-logs-info">
              <div class="request-logs-details">
                <h3 class="request-logs-title">请求记录 <span class="request-logs-count">{{ filteredRequestLogs.length
                    }}</span>
                </h3>
              </div>
            </div>
            <div class="request-logs-controls">
              <button class="config-btn clear-logs" @click="clearLogs" title="清空记录"
                :disabled="filteredRequestLogs.length === 0">
                <svg t="1761201528959" class="icon" viewBox="0 0 1024 1024" version="1.1"
                  xmlns="http://www.w3.org/2000/svg" p-id="8805" width="20" height="20">
                  <path
                    d="M38.4 170.666667h947.2a38.4 38.4 0 1 1 0 76.8H38.4A38.4 38.4 0 1 1 38.4 170.666667z m341.333333-170.666667h264.533334a38.4 38.4 0 1 1 0 76.8H379.733333a38.4 38.4 0 1 1 0-76.8z m0 341.333333a38.4 38.4 0 0 1 38.4 38.4v435.2a38.4 38.4 0 1 1-76.8 0V379.733333a38.4 38.4 0 0 1 38.4-38.4z m256 0a38.4 38.4 0 0 1 38.4 38.4v435.2a38.4 38.4 0 1 1-76.8 0V379.733333a38.4 38.4 0 0 1 38.4-38.4zM204.8 247.808V896A51.2 51.2 0 0 0 256 947.2h512a51.2 51.2 0 0 0 51.2-51.2V247.808H896V896a128 128 0 0 1-128 128H256a128 128 0 0 1-128-128V247.808h76.8z"
                    fill="currentColor" p-id="8806" />
                </svg>
              </button>
            </div>
          </div>

          <!-- 请求记录表格 -->
          <div class="request-table-container">
            <div v-if="filteredRequestLogs.length === 0" class="no-requests">
              <p>暂无请求记录</p>
              <p class="hint">服务收到的查询请求将实时显示在这里</p>
            </div>
            <div v-else class="request-table-scroll-wrap" ref="requestTableScrollWrap">
              <div class="request-table-content" ref="requestTableContent" @scroll="onRequestTableScroll">
                <table class="request-table">
                  <thead>
                    <tr>
                      <th>时间</th>
                      <th>状态</th>
                      <th>IP地址</th>
                      <th>问题</th>
                      <th>响应时间</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="log in filteredRequestLogs" :key="log.id"
                      :class="['request-row', getStatusClass(log.status), { 'selected': selectedLog?.id === log.id }]"
                      @click="showRequestDetails(log)">
                      <td class="timestamp">{{ formatTime(log.timestamp) }}</td>
                      <td class="status">
                        <span v-if="log.status" :class="['status-text', getStatusClass(log.status)]">
                          {{ log.status }}
                        </span>
                        <span v-else class="status-text pending">
                          处理中...
                        </span>
                      </td>
                      <td class="ip">{{ log.ip }}</td>
                      <td class="title" :title="getTitleFromRequestBody(log.requestBody)">
                        {{ truncateTitle(getTitleFromRequestBody(log.requestBody)) }}
                      </td>
                      <td class="response-time">
                        <span v-if="log.responseTime">{{ log.responseTime }}ms</span>
                        <span v-else class="pending">-</span>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <!-- 自定义滚动条 -->
              <div class="custom-scrollbar" :class="{ 'is-visible': requestTableScrollbarVisible }" ref="requestTableScrollbar" @mousedown="onRequestTableScrollbarMousedown">
                <div class="custom-scrollbar-thumb" ref="requestTableScrollbarThumb"></div>
              </div>
            </div>
          </div>
        </div>
      </div>

    </div>

    <!-- 右侧弹出的请求详情面板 -->
    <div v-if="showLogDetails && selectedLog" class="request-details-overlay" :class="{ 'show': slideInActive }"
      :style="{ width: overlayWidth + 'px' }">
      <!-- 拖拽条 -->
      <div class="resizer" :class="{ active: isResizing }" @mousedown="startResize">
      </div>
      <div class="request-details-header">
        <button class="back-btn" @click="closeRequestDetails">
          <svg t="1760584170728" class="icon" viewBox="0 0 1536 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
            p-id="13220" width="20" height="20">
            <path
              d="M981.418667 71.893333A60.245333 60.245333 0 0 1 1070.506667 152.746667l-3.925334 4.309333L711.594667 512l354.986666 354.944c22.186667 22.144 23.466667 57.216 3.925334 80.896l-3.925334 4.266667a60.245333 60.245333 0 0 1-80.896 3.925333l-4.266666-3.925333-368.298667-368.213334a101.632 101.632 0 0 1-4.565333-138.88l4.565333-4.864 368.298667-368.256z"
              fill="#838B9F" opacity=".25" p-id="13221"></path>
            <path
              d="M469.418667 71.893333A60.245333 60.245333 0 0 1 558.506667 152.746667l-3.925334 4.309333L199.594667 512l354.986666 354.944c22.186667 22.144 23.466667 57.216 3.925334 80.896l-3.925334 4.266667a60.245333 60.245333 0 0 1-80.896 3.925333l-4.266666-3.925333-368.298667-368.213334a101.632 101.632 0 0 1-4.565333-138.88l4.565333-4.864 368.298667-368.256z"
              fill="#838B9F" p-id="13222"></path>
          </svg>
        </button>
        <h3>请求详情</h3>
      </div>

      <div class="request-details-content">
        <!-- 标签页导航 -->
        <div class="detail-tabs">
          <button class="tab-button" :class="{ active: activeTab === 'modelResponse' }"
            @click="activeTab = 'modelResponse'">
            AI 模型响应
          </button>
          <button class="tab-button" :class="{ active: activeTab === 'requestBody' }"
            @click="activeTab = 'requestBody'">
            请求体
          </button>
          <button class="tab-button" :class="{ active: activeTab === 'responseBody' }"
            @click="activeTab = 'responseBody'">
            响应体
          </button>
          <button class="tab-button" :class="{ active: activeTab === 'basic' }" @click="activeTab = 'basic'">
            基本信息
          </button>
          <button class="tab-button" :class="{ active: activeTab === 'headers' }" @click="activeTab = 'headers'">
            请求头
          </button>
        </div>

        <!-- 标签页内容 -->
        <div class="detail-scroll-wrap">
          <div class="tab-content" ref="detailContent" @scroll="onDetailScroll">

          <!-- 基本信息 -->
          <div v-if="activeTab === 'basic'" class="detail-section">
            <div class="detail-grid">
              <div class="detail-item">
                <label>时间:</label>
                <span>{{ selectedLog ? formatTime(selectedLog.timestamp) : '' }}</span>
              </div>
              <div class="detail-item">
                <label>方法:</label>
                <span :class="['method-badge', selectedLog ? selectedLog.method.toLowerCase() : '']">{{ selectedLog ?
                  selectedLog.method : '' }}</span>
              </div>
              <div class="detail-item">
                <label>路径:</label>
                <span>{{ selectedLog ? selectedLog.path : '' }}</span>
              </div>
              <div class="detail-item">
                <label>状态码:</label>
                <span v-if="selectedLog && selectedLog.status"
                  :class="['status-text', getStatusClass(selectedLog.status)]">{{ selectedLog.status }}</span>
                <span v-else class="status-text pending">处理中...</span>
              </div>
              <div class="detail-item">
                <label>IP地址:</label>
                <span>{{ selectedLog ? selectedLog.ip : '' }}</span>
              </div>
              <div class="detail-item">
                <label>响应时间:</label>
                <span v-if="selectedLog && selectedLog.responseTime">{{ selectedLog.responseTime }}ms</span>
                <span v-else class="pending">-</span>
              </div>
            </div>
          </div>

          <!-- 请求头 -->
          <div v-if="activeTab === 'headers'" class="detail-section">
            <div v-if="selectedLog && selectedLog.headers && Object.keys(selectedLog.headers).length > 0"
              class="headers-content">
              <div v-for="(value, key) in selectedLog.headers" :key="key" class="header-item">
                <strong>{{ key }}:</strong> {{ value }}
              </div>
            </div>
            <div v-else class="no-data">
              <span class="no-data-text">暂无请求头数据</span>
            </div>
          </div>

          <!-- 请求体 -->
          <div v-show="activeTab === 'requestBody'" class="detail-section">
            <div v-if="selectedLog && selectedLog.requestBody">
              <JsonCodeViewer :content="formatJSON(selectedLog.requestBody)" />
            </div>
            <div v-else class="no-data">
              <span class="no-data-text">暂无请求体数据</span>
            </div>
          </div>

          <!-- 响应体 -->
          <div v-show="activeTab === 'responseBody'" class="detail-section">
            <div v-if="selectedLog && selectedLog.responseBody">
              <JsonCodeViewer :content="formatJSON(selectedLog.responseBody)" />
            </div>
            <div v-else class="no-data">
              <span class="no-data-text">
                <span v-if="selectedLog && !selectedLog.status">处理中，响应体暂未生成...</span>
                <span v-else>暂无响应体数据</span>
              </span>
            </div>
          </div>

          <!-- 模型响应部分 -->
          <div v-if="activeTab === 'modelResponse'" class="detail-section">

            <!-- 题目中的图片（经服务端代理转为 dataUrl 后渲染） -->
            <div v-if="selectedLog && selectedLog.hasQuestionImage" class="question-image-box">
              <div v-if="selectedLog.questionImageHtml" class="question-image-ready"
                v-html="selectedLog.questionImageHtml"></div>
              <div v-else class="loading-indicator">
                <div class="loading-spinner"></div>
                <span>图片加载中...</span>
              </div>
            </div>

            <!-- ===== 多模型切换视图 ===== -->
            <template
              v-if="selectedLog && selectedLog.multiModelResponses && selectedLog.multiModelResponses.length > 0">
              <!-- 横向模型选择器 -->
              <div class="multi-model-tabs">
                <button v-for="mr in selectedLog.multiModelResponses" :key="mr.modelId" class="multi-model-tab" :class="{
                  active: activeModelTab === mr.modelId,
                  loading: mr.isLoading,
                  summary: mr.isSummary,
                  failed: !mr.isLoading && isModelResponseErrorText(mr.response)
                }" @click="activeModelTab = mr.modelId">
                  <div v-if="mr.isLoading" class="loading-spinner-sm"></div>
                  <svg v-else-if="mr.isSummary" width="12" height="12" viewBox="0 0 1024 1024"
                    xmlns="http://www.w3.org/2000/svg">
                    <path
                      d="M303.146667 375.04A128.042667 128.042667 0 0 0 426.666667 469.333333h170.666666a213.418667 213.418667 0 0 1 210.218667 176.896A128.042667 128.042667 0 0 1 768 896a128 128 0 0 1-47.146667-247.04A128.042667 128.042667 0 0 0 597.333333 554.666667h-170.666666a212.394667 212.394667 0 0 1-128-42.666667v135.253333a128.042667 128.042667 0 1 1-85.333334 0V376.746667a128.042667 128.042667 0 1 1 89.813334-1.706667zM256 298.666667a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z m0 512a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z m512 0a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z"
                      fill="currentColor"></path>
                  </svg>
                  <span>{{ mr.modelName }}</span>
                </button>
              </div>

              <!-- 当前选中模型的响应内容 -->
              <template v-for="mr in selectedLog.multiModelResponses" :key="mr.modelId">
                <div
                  v-if="activeModelTab === mr.modelId || (!activeModelTab && selectedLog.multiModelResponses[0].modelId === mr.modelId)"
                  class="model-response-card"
                  :class="{
                    'is-loading': mr.isLoading,
                    'is-summary': mr.isSummary,
                    'is-failed': !mr.isLoading && isModelResponseErrorText(mr.response)
                  }">
                  <div class="model-response-card-header">
                    <div class="model-response-card-title">
                      <div v-if="mr.isSummary" class="summary-result-icon" title="最终总结答案">
                        <svg width="14" height="14" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
                          <path
                            d="M303.146667 375.04A128.042667 128.042667 0 0 0 426.666667 469.333333h170.666666a213.418667 213.418667 0 0 1 210.218667 176.896A128.042667 128.042667 0 0 1 768 896a128 128 0 0 1-47.146667-247.04A128.042667 128.042667 0 0 0 597.333333 554.666667h-170.666666a212.394667 212.394667 0 0 1-128-42.666667v135.253333a128.042667 128.042667 0 1 1-85.333334 0V376.746667a128.042667 128.042667 0 1 1 89.813334-1.706667zM256 298.666667a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z m0 512a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z m512 0a42.666667 42.666667 0 1 0 0-85.333334 42.666667 42.666667 0 0 0 0 85.333334z"
                            fill="currentColor"></path>
                        </svg>
                      </div>
                      <span class="card-model-name">{{ mr.modelName }}</span>
                      <span class="card-platform-name">{{ mr.platformName }}</span>
                      <span v-if="mr.isSummary" class="summary-tag">最终答案</span>
                    </div>
                    <div v-if="mr.isLoading" class="card-loading-badge">
                      <div class="loading-spinner-sm"></div>
                      <span>响应中</span>
                    </div>
                    <div v-else-if="isModelResponseErrorText(mr.response)" class="card-failed-badge">失败</div>
                    <div v-else class="card-done-badge">完成</div>
                  </div>
                  <div class="model-response-card-body">
                    <div v-if="mr.reasoningContent || mr.streamingReasoning || mr.response" class="content-stack-wrapper">
                      <AIOutputRender
                        :reasoning-content="mr.reasoningContent"
                        :streaming-reasoning="mr.streamingReasoning"
                        :response="mr.response"
                        :is-loading="mr.isLoading"
                      />
                    </div>
                    <div v-else-if="mr.isLoading" class="card-waiting">
                      <div class="loading-dots"><span></span><span></span><span></span></div>
                    </div>
                    <div v-else class="no-data"><span class="no-data-text">暂无响应</span></div>
                  </div>
                </div>
              </template>
            </template>

            <!-- ===== 单模型视图（卡片化显示） ===== -->
            <template v-else>
              <div
                v-if="selectedLog && (selectedLog.isModelCalling || selectedLog.modelResponse || selectedLog.reasoningContent)"
                class="model-response-card"
                :class="{
                  'is-loading': !!selectedLog.isModelCalling,
                  'is-failed': !selectedLog.isModelCalling && isModelResponseErrorText(selectedLog.modelResponse)
                }">
                <div class="model-response-card-header">
                  <div class="model-response-card-title">
                    <span class="card-model-name">AI 模型响应</span>
                  </div>
                  <div v-if="selectedLog.isModelCalling" class="card-loading-badge">
                    <div class="loading-spinner-sm"></div>
                    <span>响应中</span>
                  </div>
                  <div v-else-if="isModelResponseErrorText(selectedLog.modelResponse)" class="card-failed-badge">失败</div>
                  <div v-else class="card-done-badge">完成</div>
                </div>
                <div class="model-response-card-body">
                  <div v-if="selectedLog.reasoningContent || selectedLog.modelResponse" class="content-stack-wrapper">
                    <AIOutputRender
                      :reasoning-content="selectedLog.reasoningContent"
                      :response="selectedLog.modelResponse"
                      :is-loading="selectedLog.isModelCalling"
                    />
                  </div>
                  <div v-else-if="selectedLog.isModelCalling" class="card-waiting">
                    <div class="loading-dots"><span></span><span></span><span></span></div>
                  </div>
                  <div v-else class="no-data"><span class="no-data-text">暂无响应</span></div>
                </div>
              </div>
              <div v-else class="no-data">
                <span class="no-data-text">
                  <span v-if="selectedLog && !selectedLog.status">等待AI模型响应...</span>
                  <span v-else>暂无AI模型响应数据</span>
                </span>
              </div>
            </template>

          </div>
          </div>
          <div class="custom-scrollbar" :class="{ 'is-visible': scrollbarVisible, 'has-overflow': scrollbarEnabled }"
            ref="customScrollbar" @mousedown="onScrollbarMousedown">
            <div class="custom-scrollbar-thumb" ref="customScrollbarThumb"></div>
          </div>
        </div>
      </div>
    </div>

    <!-- 模型选择对话框 -->
    <ModelSelectorDialog :show="showModelSelector" :selected-text-model-ids="globalSelectedTextModels.map(m => m.id)"
      :selected-vision-model-id="globalSelectedVisionModel?.id || null"
      :selected-summary-model-ids="globalSelectedSummaryModels.map(m => m.id)" :available-models="availableModels"
      :platforms="platforms" @close="showModelSelector = false" @model-selected="selectModel" />

    <FolderPickerDialog :visible="showFolderPicker" :initial-folder-id="settings.questionSaveFolderId"
      @cancel="showFolderPicker = false" @confirm="handleFolderConfirm" />

  </div>

  <!-- OCS配置对话框 -->
  <OCSConfigDialog :visible="showOCSConfig" @close="showOCSConfig = false" />

  <ModelWarningDialog :visible="showNoModelDialog" @close="showNoModelDialog = false"
    @still-open="handleNoModelStillOpen" @select-model="handleNoModelSelect" />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, nextTick, watch } from 'vue'
import { useSettings } from '../services/settings'
import { useModelConfig } from '../services/modelConfig'
import { api, ADMIN, sseUrl, getAdminToken } from '../services/api'
import { serverRunning as globalServerRunning } from '../services/serverState'
import { findQuestionImageMatches } from '../utils/questionImage'

import ModelSelectorDialog from './home/ModelSelectorDialog.vue'
import OCSConfigDialog from './home/OCSConfigDialog.vue'
import FolderPickerDialog from '../components/FolderPickerDialog.vue'
import AIOutputRender from '../components/AIOutputRender.vue'
import ModelWarningDialog from './home/ModelWarningDialog.vue'
import JsonCodeViewer from './home/JsonCodeViewer.vue'

defineEmits(['navigate'])

// 使用设置管理器
const { settings, set, save } = useSettings()

// 使用模型配置管理器
const {
  availableModels,
  selectedTextModels: globalSelectedTextModels,
  selectedSummaryModels: globalSelectedSummaryModels,
  selectedVisionModel: globalSelectedVisionModel,
  toggleSelectedTextModel,
  toggleSelectedSummaryModel,
  toggleSelectedVisionModel,
  platforms
} = useModelConfig()

// ===== 服务状态（GET /api/status 轮询） =====
interface ServerStatus {
  status: string
  version?: string
  uptime_secs: number
  last_ocs_contact_at?: number | null
}

const serverRunning = ref(false)
watch(serverRunning, (val) => { globalServerRunning.value = val })
const serverStatus = ref<ServerStatus | null>(null)
const statusError = ref('')
let statusTimer: ReturnType<typeof setInterval> | null = null
const STATUS_POLL_INTERVAL = 10000

const fetchServerStatus = async () => {
  try {
    const res = await fetch('/api/status', { cache: 'no-store' })
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
    const data = (await res.json()) as ServerStatus
    serverStatus.value = data
    serverRunning.value = data.status === 'running'
    statusError.value = ''
  } catch (error) {
    serverRunning.value = false
    serverStatus.value = null
    statusError.value = `服务不可达: ${error instanceof Error ? error.message : String(error)}`
  }
}

const formatUptime = (secs: number) => {
  const s = Math.max(0, Math.floor(secs || 0))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return `${d}天${h}小时`
  if (h > 0) return `${h}小时${m}分`
  if (m > 0) return `${m}分${s % 60}秒`
  return `${s}秒`
}

const formatOcsContact = (ts?: number | null) => {
  if (!ts) return '尚未探测'
  return new Date(ts * 1000).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
}

// ===== 对话框状态 =====
const showModelSelector = ref(false)
const showNoModelDialog = ref(false)
const showOCSConfig = ref(false)
const showFolderPicker = ref(false)

// ===== 请求记录相关状态 =====
interface MultiModelResponse {
  modelId: string
  modelName: string
  platformName: string
  response: string
  reasoningContent?: string
  streamingReasoning?: string
  isLoading: boolean
  isSummary?: boolean
}

interface RequestLog {
  id: string
  timestamp: number
  method: string
  path: string
  status?: number // 可选，开始阶段为undefined
  ip: string
  userAgent: string
  responseTime?: number // 可选，开始阶段为undefined
  requestBody?: string
  responseBody?: string
  headers?: Record<string, string>
  stage?: string // 'started' 或 'completed'
  modelResponse?: string // 最终模型响应内容
  reasoningContent?: string // 最终思考过程
  isModelCalling?: boolean // 服务端是否正在调用模型
  multiModelResponses?: MultiModelResponse[] // 各模型并发响应（来自 model_call_progress / per_model）
  hasQuestionImage?: boolean // 题目中是否含图片 URL
  questionImageHtml?: string // 图片经服务端代理转 dataUrl 后的展示 HTML
}

const requestLogs = ref<RequestLog[]>([])
const selectedLog = ref<RequestLog | null>(null)
const showLogDetails = ref(false)
const slideInActive = ref(false)
const MAX_REQUEST_LOGS = 100

// 标签页相关状态
const activeTab = ref('modelResponse')
const activeModelTab = ref('')
const detailContent = ref<HTMLElement | null>(null)
const customScrollbar = ref<HTMLElement | null>(null)
const customScrollbarThumb = ref<HTMLElement | null>(null)
const scrollbarVisible = ref(false)
const scrollbarEnabled = ref(false)

let isDraggingScrollbar = false
let dragStartY = 0
let dragStartScrollTop = 0
let scrollHideTimer: ReturnType<typeof setTimeout> | null = null
let detailContentObserver: MutationObserver | null = null

const showScrollbar = () => {
  if (!scrollbarEnabled.value) return
  scrollbarVisible.value = true
  if (scrollHideTimer) clearTimeout(scrollHideTimer)
  scrollHideTimer = setTimeout(() => {
    scrollbarVisible.value = false
  }, 1500)
}

const updateScrollbarThumb = () => {
  const content = detailContent.value
  const thumb = customScrollbarThumb.value
  const bar = customScrollbar.value
  if (!content || !thumb || !bar) return

  const maxScroll = content.scrollHeight - content.clientHeight
  const hasOverflow = maxScroll > 0
  scrollbarEnabled.value = hasOverflow

  if (!hasOverflow) {
    thumb.style.height = '0px'
    thumb.style.transform = 'translateY(0)'
    scrollbarVisible.value = false
    return
  }

  const barHeight = bar.clientHeight || content.clientHeight
  const ratio = content.clientHeight / content.scrollHeight
  const thumbHeight = Math.max(barHeight * ratio, 36)
  const maxThumbTop = Math.max(barHeight - thumbHeight, 0)
  const thumbTop = maxScroll > 0 ? (content.scrollTop / maxScroll) * maxThumbTop : 0

  thumb.style.height = `${thumbHeight}px`
  thumb.style.transform = `translateY(${thumbTop}px)`
}

const onDetailScroll = () => {
  updateScrollbarThumb()
  showScrollbar()
}

const onScrollbarMousedown = (e: MouseEvent) => {
  const thumb = customScrollbarThumb.value
  const content = detailContent.value
  const bar = customScrollbar.value
  if (!thumb || !content || !bar || !scrollbarEnabled.value) return

  isDraggingScrollbar = true
  dragStartY = e.clientY
  dragStartScrollTop = content.scrollTop
  showScrollbar()

  const onMousemove = (event: MouseEvent) => {
    if (!isDraggingScrollbar) return
    const thumbHeight = thumb.clientHeight
    const barHeight = bar.clientHeight
    const delta = event.clientY - dragStartY
    const maxScroll = content.scrollHeight - content.clientHeight
    const maxThumbTop = Math.max(barHeight - thumbHeight, 1)
    content.scrollTop = dragStartScrollTop + (delta / maxThumbTop) * maxScroll
  }

  const onMouseup = () => {
    isDraggingScrollbar = false
    document.removeEventListener('mousemove', onMousemove)
    document.removeEventListener('mouseup', onMouseup)
  }

  document.addEventListener('mousemove', onMousemove)
  document.addEventListener('mouseup', onMouseup)
  e.preventDefault()
}

const bindDetailContentObserver = async () => {
  await nextTick()
  detailContentObserver?.disconnect()
  updateScrollbarThumb()

  if (!detailContent.value) return
  detailContentObserver = new MutationObserver(() => {
    updateScrollbarThumb()
  })
  detailContentObserver.observe(detailContent.value, {
    childList: true,
    subtree: true,
    characterData: true
  })
}

// 接收来自顶层 App 的折叠触发器，并在切换顶层 tab 时收起详情面板
const props = defineProps<{ collapseTrigger?: number }>()
watch(() => props.collapseTrigger, () => {
  if (showLogDetails.value) {
    closeRequestDetails()
  }
})

// 过滤后的请求日志（排除无任何内容的记录）
const filteredRequestLogs = computed(() => {
  return requestLogs.value.filter(log => {
    if (log.headers && Object.keys(log.headers).length > 0) return true
    return !!(log.requestBody || log.responseBody || log.modelResponse)
  })
})

// 右侧面板拖拽功能
const overlayWidth = ref(600) // 默认宽度
const isResizing = ref(false)

watch(showLogDetails, async (visible) => {
  if (visible) {
    await bindDetailContentObserver()
    showScrollbar()
    return
  }

  detailContentObserver?.disconnect()
  detailContentObserver = null
  scrollbarVisible.value = false
  scrollbarEnabled.value = false
  if (scrollHideTimer) {
    clearTimeout(scrollHideTimer)
    scrollHideTimer = null
  }
})

watch([activeTab, activeModelTab, overlayWidth, () => selectedLog.value?.id], async () => {
  if (!showLogDetails.value) return
  await bindDetailContentObserver()
})

const startResize = (event: MouseEvent) => {
  isResizing.value = true
  document.addEventListener('mousemove', handleResize)
  document.addEventListener('mouseup', stopResize)
  document.body.classList.add('resizing')
  event.preventDefault()
}

const handleResize = (event: MouseEvent) => {
  if (!isResizing.value) return

  const newWidth = window.innerWidth - event.clientX
  const minWidth = 300
  const maxWidth = window.innerWidth * 0.8 // 最大占屏幕宽度的80%

  if (newWidth >= minWidth && newWidth <= maxWidth) {
    overlayWidth.value = newWidth
  }
}

const stopResize = () => {
  isResizing.value = false
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', stopResize)
  document.body.classList.remove('resizing')
}

const isModelResponseErrorText = (text?: string | null) =>
  !!text && (text.startsWith('错误:') || text.startsWith('错误：'))

// ===== 日志数据处理 =====

/** 只关注 /query 的 GET/POST 请求 */
const isQueryLog = (data: any) =>
  data && data.path === '/query' && (data.method === 'POST' || data.method === 'GET')

/** 服务端 RequestLog（snake_case）转前端结构 */
const toRequestLog = (data: any): RequestLog => ({
  id: String(data.id),
  timestamp: new Date(data.timestamp).getTime(),
  method: data.method,
  path: data.path,
  status: typeof data.status === 'number' ? data.status : undefined,
  ip: data.ip || '-',
  userAgent: data.user_agent || 'Unknown',
  responseTime: typeof data.response_time === 'number' ? data.response_time : undefined,
  requestBody: data.request_body || '',
  responseBody: data.response_body || undefined,
  headers: data.headers || {},
  stage: data.stage || (typeof data.status === 'number' ? 'completed' : 'started')
})

/** 释放单条日志上的大字段，便于 GC */
const releaseRequestLogPayload = (log: RequestLog) => {
  log.requestBody = ''
  log.responseBody = undefined
  log.modelResponse = undefined
  log.reasoningContent = undefined
  log.multiModelResponses = undefined
  log.questionImageHtml = undefined
  log.headers = {}
}

const trimRequestLogs = () => {
  if (requestLogs.value.length <= MAX_REQUEST_LOGS) return
  const kept = requestLogs.value.slice(0, MAX_REQUEST_LOGS)
  const evicted = requestLogs.value.slice(MAX_REQUEST_LOGS)
  for (const log of evicted) releaseRequestLogPayload(log)
  if (selectedLog.value && evicted.some(log => log.id === selectedLog.value!.id)) {
    selectedLog.value = null
    showLogDetails.value = false
    slideInActive.value = false
  }
  requestLogs.value = kept
}

const findLog = (requestId: string) => requestLogs.value.find(log => log.id === requestId)

/** 处理 log 事件（started / completed 两阶段） */
const applyRequestLogEvent = (data: any) => {
  if (!isQueryLog(data)) return
  const existing = findLog(String(data.id))

  if (data.stage === 'completed' && existing) {
    // 就地更新，保持对象引用，确保 selectedLog 同步
    existing.status = data.status
    existing.responseTime = data.response_time
    existing.responseBody = data.response_body || ''
    existing.stage = 'completed'
    existing.isModelCalling = false
    if (existing.multiModelResponses) {
      existing.multiModelResponses.forEach(entry => { entry.isLoading = false })
    }
    return
  }

  if (existing) {
    // 重复的 started 事件，忽略
    return
  }

  const newLog = toRequestLog(data)
  requestLogs.value.unshift(newLog)
  void prepareQuestionImage(newLog)
  trimRequestLogs()
}

/** 题目中含图片 URL 时，通过服务端代理转为 dataUrl 生成展示 HTML */
const prepareQuestionImage = async (log: RequestLog) => {
  if (!log.requestBody) return
  let title = ''
  let options = ''
  try {
    const rb = JSON.parse(log.requestBody)
    title = rb.title || ''
    options = rb.options || ''
  } catch {
    return
  }
  const text = title + (options ? '\n\n选项：\n' + options : '')
  const matches = findQuestionImageMatches(text)
  if (matches.length === 0) return
  log.hasQuestionImage = true

  const dataUrlMap = new Map<string, string>()
  await Promise.all([...new Set(matches.map(m => m.normalizedUrl))].map(async (url) => {
    try {
      const res = await api.get<{ dataUrl: string }>(`${ADMIN}/image`, { url })
      dataUrlMap.set(url, res?.dataUrl || '')
    } catch {
      dataUrlMap.set(url, '')
    }
  }))

  let html = ''
  let lastIndex = 0
  for (const match of matches) {
    html += escapeHtml(text.slice(lastIndex, match.start))
    const dataUrl = dataUrlMap.get(match.normalizedUrl)
    html += dataUrl
      ? `<img src="${dataUrl}" style="max-width:100%;vertical-align:middle;background:#fff;display:inline-block;" />${escapeHtml(match.trailingText)}`
      : `[图片: ${escapeHtml(match.normalizedUrl)}]${escapeHtml(match.trailingText)}`
    lastIndex = match.end
  }
  html += escapeHtml(text.slice(lastIndex))
  log.questionImageHtml = html.replace(/\n/g, '<br/>')
}

const escapeHtml = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/** 获取或创建多模型条目 */
const upsertModelEntry = (log: RequestLog, modelId: string, modelName?: string, platformName?: string) => {
  if (!log.multiModelResponses) log.multiModelResponses = []
  let entry = log.multiModelResponses.find(r => r.modelId === modelId)
  if (!entry) {
    const isSummary = modelId.startsWith('summary:')
    entry = {
      modelId,
      modelName: modelName || modelId,
      platformName: platformName || '—',
      response: '',
      reasoningContent: '',
      streamingReasoning: '',
      isLoading: true,
      isSummary
    }
    log.multiModelResponses.push(entry)
    if (selectedLog.value?.id === log.id && !activeModelTab.value) {
      activeModelTab.value = modelId
    }
  } else {
    if (modelName && entry.modelName === entry.modelId) entry.modelName = modelName
    if (platformName && entry.platformName === '—') entry.platformName = platformName
  }
  return entry
}

/** model_call_request：服务端开始调用模型 */
const applyModelCallRequest = (data: any) => {
  const log = findLog(String(data.request_id || ''))
  if (!log) return
  log.isModelCalling = true
}

/** model_call_progress：按 model_id 更新流式进度 */
const applyModelCallProgress = (data: any) => {
  const log = findLog(String(data.request_id || ''))
  if (!log) return
  log.isModelCalling = true
  const content = typeof data.content === 'string' ? data.content : ''
  const reasoning = typeof data.reasoning_content === 'string' ? data.reasoning_content : ''

  if (data.model_id) {
    const entry = upsertModelEntry(log, String(data.model_id), data.model_name)
    entry.isLoading = true
    if (content) entry.response = content
    if (reasoning) entry.streamingReasoning = reasoning
    return
  }

  // 无 model_id 的整体进度（单模型场景）
  if (content) log.modelResponse = content
  if (reasoning) log.reasoningContent = reasoning
}

/** model_call_response：最终结果与各模型明细 */
const applyModelCallResponse = (data: any) => {
  const log = findLog(String(data.request_id || ''))
  if (!log) return

  log.isModelCalling = false
  const isSuccess = data.is_success !== false
  const content = typeof data.content === 'string' ? data.content : ''
  log.modelResponse = !isSuccess && content && !isModelResponseErrorText(content) ? `错误: ${content}` : content
  if (typeof data.reasoning_content === 'string' && data.reasoning_content) {
    log.reasoningContent = data.reasoning_content
  }

  const perModel = Array.isArray(data.per_model) ? data.per_model : []
  for (const item of perModel) {
    if (!item || !item.model_id) continue
    const entry = upsertModelEntry(log, String(item.model_id), item.model_name, item.platform_name)
    entry.isLoading = false
    entry.isSummary = !!item.is_summary || entry.isSummary
    if (item.model_name) entry.modelName = item.model_name
    if (item.platform_name) entry.platformName = item.platform_name
    const errorText = typeof item.error === 'string' && item.error.trim() ? item.error.trim() : ''
    const itemContent = typeof item.content === 'string' ? item.content : ''
    entry.response = errorText
      ? (isModelResponseErrorText(errorText) ? errorText : `错误: ${errorText}`)
      : itemContent || entry.response
    const finalReasoning = (typeof item.reasoning_content === 'string' && item.reasoning_content)
      || entry.streamingReasoning
      || ''
    entry.reasoningContent = finalReasoning
    entry.streamingReasoning = ''
  }

  // 未在 per_model 中出现的条目也收口
  log.multiModelResponses?.forEach(entry => {
    if (entry.isLoading) {
      entry.isLoading = false
      if (entry.streamingReasoning && !entry.reasoningContent) {
        entry.reasoningContent = entry.streamingReasoning
      }
      entry.streamingReasoning = ''
    }
  })
}

/** 初始加载最近日志 */
const loadRecentLogs = async () => {
  try {
    const logs = await api.get<any[]>(`${ADMIN}/logs/recent`)
    const list = (Array.isArray(logs) ? logs : [])
      .filter(isQueryLog)
      .map(toRequestLog)
      .sort((a, b) => b.timestamp - a.timestamp)
    requestLogs.value = list
    trimRequestLogs()
    for (const log of requestLogs.value) void prepareQuestionImage(log)
  } catch (error) {
    console.error('获取请求日志失败:', error)
  }
}

// ===== SSE 连接 =====
let sseEventSource: EventSource | null = null
let sseReconnectTimer: ReturnType<typeof setTimeout> | null = null
let sseStopped = false
const SSE_RECONNECT_DELAY = 3000

const parseSseData = (event: MessageEvent): any | null => {
  try {
    return JSON.parse(event.data)
  } catch (error) {
    console.error('解析 SSE 数据失败:', error)
    return null
  }
}

const startSSEConnection = () => {
  if (sseStopped) return
  stopSSEConnection(false)

  const source = new EventSource(sseUrl(`${ADMIN}/logs/stream`))
  sseEventSource = source

  source.addEventListener('log', (event) => {
    const data = parseSseData(event as MessageEvent)
    if (data) applyRequestLogEvent(data)
  })
  source.addEventListener('model_call_request', (event) => {
    const data = parseSseData(event as MessageEvent)
    if (data) applyModelCallRequest(data)
  })
  source.addEventListener('model_call_progress', (event) => {
    const data = parseSseData(event as MessageEvent)
    if (data) applyModelCallProgress(data)
  })
  source.addEventListener('model_call_response', (event) => {
    const data = parseSseData(event as MessageEvent)
    if (data) applyModelCallResponse(data)
  })
  source.addEventListener('ocs_head', () => {
    // OCS 探测：立即刷新状态卡片
    void fetchServerStatus()
  })
  source.onerror = () => {
    // 断开后 3s 重连
    source.close()
    if (sseEventSource === source) sseEventSource = null
    scheduleSseReconnect()
  }
}

const scheduleSseReconnect = () => {
  if (sseStopped || sseReconnectTimer) return
  sseReconnectTimer = setTimeout(() => {
    sseReconnectTimer = null
    startSSEConnection()
  }, SSE_RECONNECT_DELAY)
}

const stopSSEConnection = (permanent = true) => {
  if (permanent) sseStopped = true
  if (sseReconnectTimer) {
    clearTimeout(sseReconnectTimer)
    sseReconnectTimer = null
  }
  if (sseEventSource) {
    sseEventSource.close()
    sseEventSource = null
  }
}

const onVisibilityChange = () => {
  if (document.visibilityState !== 'visible') return
  // 回到前台：必要时重连 SSE 并刷新状态
  if (!sseEventSource || sseEventSource.readyState === EventSource.CLOSED) {
    startSSEConnection()
  }
  void fetchServerStatus()
}

const clearLogs = async () => {
  try {
    await api.delete(`${ADMIN}/logs`)
  } catch (error) {
    console.error('清空请求日志失败:', error)
  }
  for (const log of requestLogs.value) releaseRequestLogPayload(log)
  requestLogs.value = []
  selectedLog.value = null
  showLogDetails.value = false
  slideInActive.value = false
  activeModelTab.value = ''
}

// ===== 测试查询 =====
const testQueryTitle = ref('')
const testQueryRunning = ref(false)
const testQueryResult = ref('')
const testQueryError = ref(false)

/** 查询令牌：优先用户令牌，否则回退管理员令牌 */
const getQueryToken = () => settings.multiUser?.users?.[0]?.token || getAdminToken()

const runTestQuery = async () => {
  const title = testQueryTitle.value.trim()
  if (!title || testQueryRunning.value) return
  testQueryRunning.value = true
  testQueryResult.value = ''
  testQueryError.value = false
  try {
    const params = new URLSearchParams({ title, token: getQueryToken() })
    const res = await fetch(`/query?${params.toString()}`)
    const text = await res.text()
    testQueryResult.value = formatJSON(text)
    testQueryError.value = !res.ok
  } catch (error) {
    testQueryResult.value = `查询失败: ${error instanceof Error ? error.message : String(error)}`
    testQueryError.value = true
  } finally {
    testQueryRunning.value = false
  }
}

// ===== 格式化工具 =====
const formatTime = (timestamp: number) => {
  const date = new Date(timestamp)
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
}

const getStatusClass = (status?: number) => {
  if (!status) return 'unknown'
  if (status >= 200 && status < 300) return 'success'
  if (status >= 300 && status < 400) return 'redirect'
  if (status >= 400 && status < 500) return 'client-error'
  if (status >= 500) return 'server-error'
  return 'unknown'
}

// 请求表格自定义滚动条
const requestTableScrollWrap = ref<HTMLElement | null>(null)
const requestTableContent = ref<HTMLElement | null>(null)
const requestTableScrollbar = ref<HTMLElement | null>(null)
const requestTableScrollbarThumb = ref<HTMLElement | null>(null)
const requestTableScrollbarVisible = ref(false)
let requestTableDragStartY = 0
let requestTableDragStartScrollTop = 0
let requestTableDragging = false
let requestTableHideTimer: ReturnType<typeof setTimeout> | null = null

const showRequestTableScrollbar = () => {
  requestTableScrollbarVisible.value = true
  if (requestTableHideTimer) clearTimeout(requestTableHideTimer)
  requestTableHideTimer = setTimeout(() => {
    requestTableScrollbarVisible.value = false
  }, 1500)
}

const updateRequestTableScrollbarThumb = () => {
  const content = requestTableContent.value
  const thumb = requestTableScrollbarThumb.value
  const bar = requestTableScrollbar.value
  if (!content || !thumb || !bar) return

  const scrollRatio = content.scrollTop / (content.scrollHeight - content.clientHeight || 1)
  const thumbHeight = Math.max((content.clientHeight / content.scrollHeight) * bar.clientHeight, 20)
  const maxTop = bar.clientHeight - thumbHeight
  thumb.style.height = `${thumbHeight}px`
  thumb.style.top = `${scrollRatio * maxTop}px`
}

const onRequestTableScroll = () => {
  showRequestTableScrollbar()
  updateRequestTableScrollbarThumb()
}

const onRequestTableScrollbarMousedown = (e: MouseEvent) => {
  e.preventDefault() // 阻止默认的文本选择行为
  const thumb = requestTableScrollbarThumb.value
  const content = requestTableContent.value
  const wrap = requestTableScrollWrap.value
  if (!thumb || !content) return

  requestTableDragging = true
  requestTableDragStartY = e.clientY
  requestTableDragStartScrollTop = content.scrollTop

  if (wrap) wrap.classList.add('is-dragging')
  document.body.style.userSelect = 'none' // 拖拽时全局禁用文本选择

  const onMouseMove = (e: MouseEvent) => {
    if (!requestTableDragging) return
    const deltaY = e.clientY - requestTableDragStartY

    const bar = requestTableScrollbar.value
    const thumbHeight = thumb.clientHeight
    const maxTop = (bar?.clientHeight || content.clientHeight) - thumbHeight

    const scrollRatio = deltaY / (maxTop || 1)
    content.scrollTop = requestTableDragStartScrollTop + scrollRatio * (content.scrollHeight - content.clientHeight)
    showRequestTableScrollbar()
    updateRequestTableScrollbarThumb()
  }

  const onMouseUp = () => {
    requestTableDragging = false
    if (wrap) wrap.classList.remove('is-dragging')
    document.body.style.userSelect = '' // 恢复文本选择
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
  }

  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp)
}

// 监听请求日志变化，更新滚动条
watch(filteredRequestLogs, async () => {
  await nextTick()
  updateRequestTableScrollbarThumb()
  if (requestTableContent.value) {
    showRequestTableScrollbar()
  }
}, { deep: true })

// 从请求体中提取title参数
const getTitleFromRequestBody = (requestBody?: string) => {
  if (!requestBody) return 'Unknown'

  try {
    const parsed = JSON.parse(requestBody)
    return parsed.title || 'Unknown'
  } catch {
    return 'Unknown'
  }
}

// 截断标题字符串
const truncateTitle = (title: string) => {
  if (title.length <= 50) return title
  return title.substring(0, 47) + '...'
}

// 显示请求详情
const showRequestDetails = (log: RequestLog) => {
  const alreadyOpen = showLogDetails.value && slideInActive.value
  selectedLog.value = log
  activeModelTab.value = log.multiModelResponses?.[0]?.modelId || ''

  // 已打开时只切换内容，避免关闭再滑入造成闪动
  if (alreadyOpen) {
    return
  }

  showLogDetails.value = true
  slideInActive.value = false
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      slideInActive.value = true
    })
  })
}

// 关闭请求详情
const closeRequestDetails = () => {
  slideInActive.value = false
  // 等待动画完成后再隐藏元素
  setTimeout(() => {
    showLogDetails.value = false
    selectedLog.value = null
  }, 300) // 与CSS动画时间一致
}

// 格式化JSON字符串
const formatJSON = (jsonString: string) => {
  try {
    const parsed = JSON.parse(jsonString)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return jsonString
  }
}

// ===== 配置入口 =====
const openOCSConfig = () => {
  showOCSConfig.value = true
}

const openModelSelector = () => {
  showModelSelector.value = true
}

const openFolderPicker = () => {
  showFolderPicker.value = true
}

const handleFolderConfirm = async (folderId: number, folderName: string, folderPath: string) => {
  showFolderPicker.value = false
  set('questionSaveDir', folderPath || folderName)
  set('questionSaveFolderId', folderId)
  await save()
}

// 模型选择：写回模型配置即可，服务端自动生效
const selectModel = (model: any) => {
  const category = model.category || 'text'

  if (category === 'summary') {
    toggleSelectedSummaryModel(model.id)
  } else if (category === 'vision') {
    toggleSelectedVisionModel(model.id)
  } else {
    toggleSelectedTextModel(model.id)
  }
}

const handleNoModelStillOpen = (dontRemind: boolean) => {
  if (dontRemind) {
    set('suppressNoModelWarning', true)
    save()
  }
  showNoModelDialog.value = false
}

const handleNoModelSelect = (dontRemind: boolean) => {
  if (dontRemind) {
    set('suppressNoModelWarning', true)
    save()
  }
  showNoModelDialog.value = false
  showModelSelector.value = true
}

// 未选择文本模型时提示一次（配置加载后再判断）
let noModelWarned = false
const warnIfNoModel = () => {
  if (noModelWarned) return
  if (platforms.value.length === 0) return
  noModelWarned = true
  if (globalSelectedTextModels.value.length === 0 && !settings.suppressNoModelWarning) {
    showNoModelDialog.value = true
  }
}
watch(platforms, warnIfNoModel)

// ===== 生命周期 =====
onMounted(async () => {
  window.addEventListener('open-ocs-config', openOCSConfig)
  document.addEventListener('visibilitychange', onVisibilityChange)

  await fetchServerStatus()
  statusTimer = setInterval(fetchServerStatus, STATUS_POLL_INTERVAL)

  await loadRecentLogs()
  sseStopped = false
  startSSEConnection()
  warnIfNoModel()
})

onUnmounted(() => {
  window.removeEventListener('open-ocs-config', openOCSConfig)
  document.removeEventListener('visibilitychange', onVisibilityChange)
  if (statusTimer) {
    clearInterval(statusTimer)
    statusTimer = null
  }
  stopSSEConnection()
  detailContentObserver?.disconnect()
  if (scrollHideTimer) clearTimeout(scrollHideTimer)
  if (requestTableHideTimer) clearTimeout(requestTableHideTimer)
  for (const log of requestLogs.value) releaseRequestLogPayload(log)
  requestLogs.value = []
  selectedLog.value = null
})
</script>

<style scoped>
.home-page {
  background-color: var(--bg-secondary);
  border-radius: 4px;
  height: calc(100% - 5px);
  width: calc(100% - 5px);
  margin: 0 2px 2px 0;
  padding: 20px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 20px;
  min-height: 0;
  overflow: hidden;
  padding-bottom: 0px;
}

.server-control {
  flex-shrink: 0;
  margin-bottom: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.server-card {
  margin: 0;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 30px;
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.server-main-content {
  border-radius: 12px;
  padding: 6px 10px;
  border: 1px solid var(--bg-tertiary);
  display: flex;
  align-items: center;
  gap: 20px;
  flex: 1;
  min-width: 0;
}

.server-config-controls {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-shrink: 0;
}

.server-action {
  display: flex;
  align-items: center;
}

.server-info {
  display: flex;
  align-items: center;
  gap: 20px;
  flex: 1;
  min-width: 0;
}

.server-icon {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  flex-shrink: 0;
  transition: all 0.3s ease;
}

.server-icon.running {
  background: #48bb78;
  box-shadow: 0 0 8px rgba(72, 187, 120, 0.4);
}

.server-icon.stopped {
  background: #e53e3e;
  box-shadow: 0 0 8px rgba(229, 62, 62, 0.4);
}

.server-details {
  flex: 1;
  min-width: 0;
}

.server-url {
  margin-top: 0px;
  font-size: 14px;
  color: #718096;
  margin-bottom: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.server-status-text {
  color: var(--color-primary);
  font-weight: 500;
}

.server-meta {
  color: var(--text-secondary);
  font-size: 13px;
  white-space: nowrap;
}

.url-placeholder {
  color: #a0aec0;
  font-weight: 500;
}

/* 测试查询 */
.test-query-form {
  display: flex;
  align-items: center;
  gap: 8px;
}

.test-query-input {
  width: 220px;
  height: 40px;
  padding: 0 12px;
  border: 1px solid var(--bg-tertiary);
  border-radius: 8px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 13px;
  outline: none;
  transition: border-color 0.2s ease;
}

.test-query-input:focus {
  border-color: var(--color-primary);
}

.test-query-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 12px 24px;
  border: none;
  border-radius: 8px;
  background: var(--server-toggle-bg);
  color: var(--server-toggle-text);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  min-width: 110px;
  justify-content: center;
}

.test-query-btn:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 4px 16px var(--server-toggle-shadow);
  background-color: var(--server-toggle-hover-bg);
  color: var(--server-toggle-hover-text);
}

.test-query-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.test-query-btn.loading {
  pointer-events: none;
}

.test-query-result {
  border-radius: 8px;
  border: 1px solid var(--bg-tertiary);
  background: var(--bg-primary);
  max-height: 160px;
  overflow: auto;
  padding: 10px 12px;
}

.test-query-result.error {
  border-color: rgba(229, 62, 62, 0.4);
}

.test-query-result pre {
  margin: 0;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-primary);
  white-space: pre-wrap;
  word-break: break-all;
}

.loading-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid transparent;
  border-top: 2px solid currentColor;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.config-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 100%;
  border-radius: 8px;
  background: var(--bg-secondary);
  color: #718096;
  cursor: pointer;
  transition: all 0.2s ease;
  border: none;
}

.config-btn svg {
  width: 20px !important;
  height: 20px !important;
  fill: currentColor;
  flex-shrink: 0;
  display: block;
}

.config-btn.model-config svg {
  width: 20px !important;
  height: 20px !important;
  fill: #9ca3af;
  flex-shrink: 0;
  display: block;
}

.config-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

@media (max-width: 768px) {
  .home-page {
    padding: 16px;
    gap: 16px;
  }
}

/* 请求记录布局样式 */
.request-logs-layout {
  display: flex;
  flex: 1;
  min-height: 0;
  gap: 4px;
  overflow: hidden;
}

.request-logs-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  overflow: hidden;
  transition: width 0.3s ease;
}

.request-logs-card {
  background: var(--bg-secondary);
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.request-logs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
}

.request-logs-info {
  display: flex;
  align-items: center;
  gap: 20px;
}

.request-logs-details {
  flex: 1;
}

.request-logs-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.request-logs-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 20px;
  padding: 0 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 500;
  border-radius: 10px;
}

.request-logs-controls {
  display: flex;
  gap: 8px;
  align-items: center;
}

.request-table-container {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.request-table-scroll-wrap {
  position: relative;
  flex: 1;
  display: flex;
  overflow: hidden;
}

.request-table-scroll-wrap.is-dragging .request-table {
  pointer-events: none; /* 拖拽时禁用表格事件，防止和hover冲突 */
}

.request-table-content {
  overflow-y: auto;
  flex: 1;
  scrollbar-width: none;
  -ms-overflow-style: none;
  padding-right: 10px; /* 给右侧滚动条留出空间，防止和表格横线内容重叠 */
}

.request-table-content::-webkit-scrollbar { display: none; }
.request-table-content::-webkit-scrollbar-button { display: none; }

.custom-scrollbar {
  position: absolute;
  right: 3px;
  top: 4px;
  bottom: 4px;
  width: 4px;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.2s;
  cursor: pointer;
  pointer-events: none;
}

.custom-scrollbar.is-visible {
  opacity: 1;
  pointer-events: auto;
}

.custom-scrollbar-thumb {
  width: 4px;
  border-radius: 4px;
  background: var(--custom-scrollbar-thumb);
  transition: background 0.15s;
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
}

.custom-scrollbar-thumb:hover {
  background: var(--custom-scrollbar-thumb-hover);
}

.custom-scrollbar:hover .custom-scrollbar-thumb {
  background: var(--text-tertiary);
}

.request-table {
  width: 100%;
  min-width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 13px;
  table-layout: fixed;
  color: var(--text-primary);
}

.request-table thead {
  border-bottom: 2px solid var(--border-primary);
}

.request-table th {
  padding: 12px 8px;
  text-align: left;
  font-weight: 500;
  color: var(--ql-th-text);
  border-bottom: 1px solid var(--border-primary);
  white-space: nowrap;
  position: relative;
}

.request-table th:not(:last-child)::after {
  content: '';
  position: absolute;
  right: 0;
  top: 20%;
  bottom: 20%;
  width: 1px;
  background: var(--ql-divider);
}

/* 时间 */
.request-table th:nth-child(1) {
  width: 120px;
}

/* 状态 */
.request-table th:nth-child(2) {
  width: 80px;
}

/* IP地址 */
.request-table th:nth-child(3) {
  width: 120px;
}

/* 问题 */
.request-table th:nth-child(4) {
  width: auto;
}

/* 响应时间 */
.request-table th:nth-child(5) {
  width: 100px;
}

.request-table tbody tr {
  cursor: pointer;
  transition: background-color 0.2s ease;
  position: relative;
}

.request-table tbody tr:hover {
  background-color: var(--ql-row-hover-bg);
}

.request-table td {
  padding: 12px 8px;
  border-bottom: 1px solid var(--border-primary);
  vertical-align: middle;
  word-wrap: break-word;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  position: relative;
}

.request-table tbody tr td:not(:last-child)::after {
  content: '';
  position: absolute;
  right: 0;
  top: 20%;
  bottom: 20%;
  width: 1px;
  background: var(--ql-table-divider);
  opacity: 0.6;
}

.request-table tbody tr:hover td:not(:last-child)::after {
  background: var(--ql-table-divider-hover);
  opacity: 0.8;
}

.request-table tbody tr:last-child td {
  border-bottom: none;
}

.no-requests {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: #a0aec0;
  padding: 40px 20px;
}

.status-text {
  font-size: 14px;
  font-weight: 600;
  font-family: 'Courier New', monospace;
}

.status-text.success {
  color: #38a169;
}

.status-text.redirect {
  color: #d69e2e;
}

.status-text.client-error {
  color: #e53e3e;
}

.status-text.server-error {
  color: #e53e3e;
}

.status-text.unknown {
  color: #718096;
}

.status-text.pending {
  color: #3182ce;
  animation: pulse 2s infinite;
}

@keyframes pulse {

  0%,
  100% {
    opacity: 1;
  }

  50% {
    opacity: 0.6;
  }
}

.pending {
  color: #718096;
  font-style: italic;
}

.ip {
  font-family: 'Courier New', monospace;
  font-size: 13px;
}

/* 右侧弹出请求详情面板样式 */
.request-details-overlay {
  position: fixed;
  top: 0;
  right: 0;
  width: 50%;
  height: 100vh;
  background-color: var(--bg-secondary);
  border-left: 1px solid var(--bg-primary);
  box-shadow: -2px 0 8px var(--request-details-shadow);
  z-index: 1000;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* 初始状态：完全隐藏在右侧 */
  transform: translateX(100%);
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

/* 显示状态：滑入到正常位置 */
.request-details-overlay.show {
  transform: translateX(0);
}

/* 拖拽条样式 */
.resizer {
  position: absolute;
  left: 0;
  top: 0;
  width: 4px;
  height: 100%;
  background-color: transparent;
  cursor: ew-resize;
  z-index: 1001;
  transition: background-color 0.2s ease;
}

.resizer:hover {
  background-color: var(--request-details-resizer-hover);
}

.resizer.active {
  background-color: var(--request-details-resizer-active);
}

/* 拖拽时的全局样式 */
body.resizing {
  cursor: ew-resize !important;
  user-select: none;
}

.request-details-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  gap: 12px;
  flex-shrink: 0;
}

.request-details-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--request-details-header-text);
  flex: 1;
}

.back-btn {
  background: none;
  border: none;
  cursor: pointer;
  padding: 8px;
  border-radius: 6px;
  transition: background-color 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--request-details-back-btn-text);
  /* 图标路径默认朝左；用 CSS 翻转（勿写在 SVG transform 上，Mac/Windows 表现不一致） */
  transform: scaleX(-1);
}

.back-btn:hover {
  background-color: var(--request-details-back-btn-hover);
}

.request-details-content {
  height: 100%;
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow-y: hidden;
  /* 仅让下方内容区滚动，标签固定 */
  padding: 0;
}

/* 标签页导航样式 */
.detail-tabs {
  display: flex;
  border-bottom: 1px solid var(--bg-primary);
  margin-bottom: 0;
  gap: 4px;
  padding: 0 24px;
}

.tab-button {
  padding: 12px 20px;
  border: none;
  background: transparent;
  color: var(--request-details-tab-text);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  border-radius: 8px 8px 0 0;
  transition: all 0.2s ease;
  position: relative;
  overflow: hidden;
}

.tab-button::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 50%;
  width: 0;
  height: 3px;
  background: var(--request-details-tab-active);
  transition: all 0.2s ease;
  transform: translateX(-50%);
}

.tab-button:hover::after {
  width: 20%;
}

.tab-button.active::after {
  width: 40%;
}

/* 标签页内容样式 */
.detail-scroll-wrap {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.tab-content {
  min-height: 0;
  height: 100%;
  box-sizing: border-box;
  overflow-y: auto;
  padding: 24px;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.tab-content::-webkit-scrollbar,
.tab-content::-webkit-scrollbar-button {
  display: none;
}

.detail-scroll-wrap .custom-scrollbar {
  position: absolute;
  top: 8px;
  right: 6px;
  bottom: 8px;
  width: 6px;
  border-radius: 999px;
  opacity: 0;
  transition: opacity 0.18s ease;
  pointer-events: none;
  z-index: 2;
}

.detail-scroll-wrap .custom-scrollbar.has-overflow {
  pointer-events: auto;
}

.detail-scroll-wrap .custom-scrollbar.is-visible.has-overflow {
  opacity: 1;
}

.detail-scroll-wrap .custom-scrollbar-thumb {
  position: absolute;
  top: 0;
  right: 1px;
  width: 4px;
  border-radius: 4px;
  background: var(--custom-scrollbar-thumb);
  transition: background 0.15s ease;
}

/* 选中行样式 */
.request-row.selected {
  background: var(--ql-row-active-bg) !important;
}

.request-row.selected td:not(:last-child)::after {
  background: var(--ql-table-divider-active);
  opacity: 1;
}

.request-row.selected:hover {
  background: var(--ql-row-active-bg) !important;
}

.detail-section {
  margin-bottom: 24px;
}

.detail-section:last-child {
  margin-bottom: 0;
}

.detail-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 16px;
}

.detail-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.detail-item label {
  font-weight: 500;
  color: var(--request-details-label-text);
  min-width: 80px;
}

.detail-item span {
  color: var(--request-details-value-text);
}

.headers-content {
  background: var(--request-details-headers-bg);
  border-radius: 8px;
  padding: 16px;
  border: 1px solid var(--request-details-headers-border);
}

.header-item {
  margin-bottom: 8px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  color: var(--request-details-headers-text);
}

.header-item:last-child {
  margin-bottom: 0;
}

.header-item strong {
  color: var(--request-details-headers-strong);
}

.loading-indicator {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: center;
  color: var(--text-secondary);
  font-size: 13px;
}

/* 无数据提示样式 */
.no-data {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 120px;
  background: var(--request-details-headers-bg);
  border-radius: 8px;
  border: 1px solid var(--request-details-headers-border);
}

.no-data-text {
  color: var(--request-details-label-text);
  font-size: 14px;
  opacity: 0.7;
  font-style: italic;
}

/* Tooltip 样式 */
.tooltip-container {
  height: 32px;
  position: relative;
  display: inline-block;
  transition: all 0.3s ease;
}

.tooltip {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  padding: 0.3em 0.6em;
  background-color: rgb(255, 255, 255);
  color: rgb(34, 34, 34);
  border-radius: 12px;
  font-size: 12px;
  white-space: nowrap;
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transition: all 0.3s ease;
  z-index: 1000;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.tooltip::before {
  position: absolute;
  content: "";
  height: 0.6em;
  width: 0.6em;
  top: -0.4em;
  left: 50%;
  transform: translate(-50%) rotate(45deg);
  background-color: rgb(255, 255, 255);
  border-left: 1px solid rgba(0, 0, 0, 0.1);
  border-top: 1px solid rgba(0, 0, 0, 0.1);
}

.tooltip-container:hover .tooltip {
  bottom: -160%;
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
}

/* 响应式设计 */
@media (max-width: 1024px) {
  .title {
    max-width: 150px;
  }
}

@media (max-width: 768px) {
  .title {
    max-width: 100px;
  }

  .server-card {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .test-query-input {
    flex: 1;
    width: auto;
  }
}

/* 横向模型切换 tab */
.multi-model-tabs {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.multi-model-tab {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border-radius: 20px;
  background: var(--multi-model-tab-bg);
  color: var(--multi-model-tab-text);
  box-shadow: var(--multi-model-tab-shadow);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease, border-color 0.18s ease, box-shadow 0.18s ease, transform 0.18s ease;
  white-space: nowrap;
  border: none;
}

.multi-model-tab:hover {
  background: var(--multi-model-tab-hover-bg);
  border-color: var(--multi-model-tab-hover-border);
  color: var(--multi-model-tab-hover-text);
  box-shadow: var(--multi-model-tab-hover-shadow);
  transform: translateY(-1px);
}

.multi-model-tab.active {
  background: var(--multi-model-tab-active-bg);
  border-color: var(--multi-model-tab-active-border);
  color: var(--multi-model-tab-active-text);
  box-shadow: var(--multi-model-tab-active-shadow);
}

.multi-model-tab.summary {
  background: var(--multi-model-tab-summary-bg);
  color: var(--multi-model-tab-summary-text);
  box-shadow: none;
}

.multi-model-tab.summary:hover {
  background: var(--multi-model-tab-summary-hover-bg);
  border-color: var(--multi-model-tab-summary-hover-border);
  color: var(--multi-model-tab-summary-hover-text);
  box-shadow: none;
}

.multi-model-tab.summary.active {
  background: var(--multi-model-tab-summary-active-bg);
  border-color: var(--multi-model-tab-summary-active-border);
  color: var(--multi-model-tab-summary-active-text);
  box-shadow: var(--multi-model-tab-active-shadow), var(--multi-model-tab-summary-active-glow);
}

.multi-model-tab.loading {
  opacity: 0.75;
}

.multi-model-tab.failed {
  background: var(--multi-model-tab-failed-bg);
  color: var(--multi-model-tab-failed-text);
  box-shadow: none;
}

.multi-model-tab.failed:hover {
  background: var(--multi-model-tab-failed-hover-bg);
  color: var(--multi-model-tab-failed-hover-text);
  box-shadow: inset 0 0 0 1px var(--multi-model-tab-failed-hover-border), var(--multi-model-tab-failed-shadow);
}

.multi-model-tab.failed.active {
  background: var(--multi-model-tab-failed-active-bg);
  color: var(--multi-model-tab-failed-active-text);
  box-shadow: inset 0 0 0 1px var(--multi-model-tab-failed-active-border), var(--multi-model-tab-failed-active-shadow);
}

.model-response-card {
  border-radius: 10px;
  overflow: hidden;
  border: 1px solid transparent;
  background: var(--model-response-card-bg);
  transition: border-color 0.2s, box-shadow 0.2s, background-color 0.2s;
}

.model-response-card.is-loading {
  border-color: transparent;
  box-shadow: none;
}

.model-response-card.is-summary {
  background: var(--model-response-card-summary-bg);
}

.model-response-card.is-summary .model-response-card-header {
  background: var(--model-response-card-summary-header-bg);
}

.model-response-card.is-summary .model-response-card-header::after {
  background: var(--model-response-card-summary-divider);
}

.summary-result-icon {
  color: var(--model-response-card-summary-accent);
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.summary-tag {
  background: var(--model-response-card-summary-tag-bg);
  color: var(--model-response-card-summary-tag-text);
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  white-space: nowrap;
}

.model-response-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  gap: 12px;
  position: relative;
}

.model-response-card-header::after {
  content: '';
  position: absolute;
  left: 16px;
  right: 16px;
  bottom: 0;
  height: 1px;
  background: var(--model-response-card-divider);
}

.model-response-card-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.card-model-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--model-response-card-title);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-platform-name {
  font-size: 11px;
  color: var(--model-response-card-muted-text);
  opacity: 0.7;
  white-space: nowrap;
}

.card-loading-badge {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: var(--model-response-card-muted-text);
  flex-shrink: 0;
}

.card-failed-badge {
  font-size: 12px;
  color: #ef4444;
  font-weight: 600;
  flex-shrink: 0;
  white-space: nowrap;
}

.model-response-card.is-failed {
  box-shadow: inset 0 0 0 1px rgba(239, 68, 68, 0.28);
}

.card-done-badge {
  font-size: 12px;
  color: var(--model-response-card-success-text);
  font-weight: 500;
  flex-shrink: 0;
}

.model-response-card-body {
  padding: 14px 16px;
}

/* 卡片头部小型加载动画 */
.loading-spinner-sm {
  width: 12px;
  height: 12px;
  border: 2px solid var(--model-response-card-spinner-track);
  border-top-color: var(--model-response-card-spinner-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}

/* 卡片等待状态的加载点 */
.card-waiting {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 60px;
}

.loading-dots {
  display: flex;
  gap: 6px;
  align-items: center;
}

.loading-dots span {
  width: 7px;
  height: 7px;
  background: var(--model-response-card-muted-text);
  border-radius: 50%;
  opacity: 0.4;
  animation: dot-pulse 1.2s ease-in-out infinite;
}

.loading-dots span:nth-child(2) {
  animation-delay: 0.2s;
}

.loading-dots span:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes dot-pulse {

  0%,
  80%,
  100% {
    opacity: 0.4;
    transform: scale(0.8);
  }

  40% {
    opacity: 1;
    transform: scale(1.1);
  }
}

/* 题目图片展示 */
.question-image-box {
  background: var(--bg-tertiary, #f8f9fa);
  border: 1px solid var(--border-color, #e0e0e0);
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 16px;
  font-size: 14px;
  line-height: 1.7;
  color: var(--text-primary);
  word-break: break-word;
}
</style>
